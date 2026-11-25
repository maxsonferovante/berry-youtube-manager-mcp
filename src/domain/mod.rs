use anyhow::{Context, Result, bail};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Privacy {
    Private,
    Unlisted,
    Public,
}
impl Privacy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Unlisted => "unlisted",
            Self::Public => "public",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VideoRequest {
    /// Caminho absoluto do vídeo local.
    pub file_path: String,
    /// Título do vídeo.
    pub title: String,
    /// Descrição do vídeo.
    pub description: Option<String>,
    /// private, unlisted ou public. O padrão é public, ou private ao agendar.
    pub privacy: Option<Privacy>,
    /// Data de publicação em RFC 3339; a privacidade deve ser private ou omitida.
    pub publish_at: Option<String>,
    /// Categoria do YouTube, por exemplo 10 para música.
    pub category_id: Option<String>,
    pub tags: Option<Vec<String>>,
    /// Declare se o vídeo é destinado a crianças.
    pub made_for_kids: bool,
    /// Se o vídeo contém conteúdo realista alterado ou sintético; omitir significa não declarar.
    pub contains_synthetic_media: Option<bool>,
    /// Faixas de legenda opcionais, em arquivos locais com marcação de tempo (ex.: SRT ou VTT).
    pub captions: Option<Vec<CaptionTrack>>,
}
impl VideoRequest {
    pub fn effective_privacy(&self) -> &'static str {
        self.privacy.as_ref().map_or_else(
            || {
                if self.publish_at.is_some() {
                    "private"
                } else {
                    "public"
                }
            },
            Privacy::as_str,
        )
    }

    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() || self.title.chars().count() > 100 {
            bail!("o título deve ter entre 1 e 100 caracteres");
        }
        if !std::path::Path::new(&self.file_path).is_absolute() {
            bail!("file_path deve ser absoluto");
        }
        if self.publish_at.is_some() && !matches!(self.privacy, None | Some(Privacy::Private)) {
            bail!("publish_at exige privacy private");
        }
        if let Some(captions) = &self.captions {
            if captions.is_empty() {
                bail!("captions deve conter pelo menos uma faixa ou ser omitido");
            }
            for (i, caption) in captions.iter().enumerate() {
                caption.validate()?;
                if captions[..i].iter().any(|previous| {
                    previous.language == caption.language && previous.name == caption.name
                }) {
                    bail!("faixas de legenda não podem repetir idioma e nome");
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptionTrack {
    /// Caminho absoluto de um arquivo de legendas com marcação de tempo.
    pub file_path: String,
    /// Idioma da faixa, por exemplo pt-BR ou en.
    pub language: String,
    /// Nome que identifica a faixa no idioma.
    pub name: String,
    /// Se verdadeira, mantém a faixa como rascunho; padrão false.
    pub is_draft: Option<bool>,
}
impl CaptionTrack {
    pub fn validate(&self) -> Result<()> {
        if !std::path::Path::new(&self.file_path).is_absolute() {
            bail!("o caminho da legenda deve ser absoluto");
        }
        if self.language.trim().is_empty() || self.name.trim().is_empty() {
            bail!("o idioma e o nome da legenda são obrigatórios");
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptionRequest {
    pub video_id: String,
    pub caption: CaptionTrack,
}
impl CaptionRequest {
    pub fn validate(&self) -> Result<()> {
        validate_video_id(&self.video_id)?;
        self.caption.validate()
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StatisticsResource {
    Video,
    Channel,
    Playlist,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StatisticsRequest {
    /// Tipo do recurso: video, channel ou playlist.
    pub resource: StatisticsResource,
    /// Até 50 IDs. Em channel, omita para consultar o canal autenticado.
    pub ids: Option<Vec<String>>,
}
impl StatisticsRequest {
    pub fn validate(&self) -> Result<()> {
        match self.ids.as_deref() {
            None if matches!(self.resource, StatisticsResource::Channel) => Ok(()),
            Some(ids) if (1..=50).contains(&ids.len()) => {
                for id in ids {
                    if matches!(self.resource, StatisticsResource::Video) {
                        validate_video_id(id)?;
                    } else {
                        validate_resource_id(id)?;
                    }
                }
                Ok(())
            }
            _ => bail!("informe entre 1 e 50 IDs; apenas channel permite omitir ids"),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnalyticsRequest {
    /// channel==MINE por padrão; também aceita channel==ID ou contentOwner==ID.
    pub ids: Option<String>,
    /// Datas inclusivas no formato YYYY-MM-DD.
    pub start_date: String,
    pub end_date: String,
    /// Métricas da YouTube Analytics API separadas por vírgula.
    pub metrics: String,
    pub dimensions: Option<String>,
    pub filters: Option<String>,
    pub sort: Option<String>,
    pub currency: Option<String>,
    pub max_results: Option<u32>,
    /// Índice inicial baseado em 1 para paginação.
    pub start_index: Option<u32>,
    pub include_historical_channel_data: Option<bool>,
}
impl AnalyticsRequest {
    pub fn validate(&self) -> Result<()> {
        let start = chrono::NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d")?;
        let end = chrono::NaiveDate::parse_from_str(&self.end_date, "%Y-%m-%d")?;
        if start > end {
            bail!("start_date não pode ser posterior a end_date");
        }
        if self.metrics.trim().is_empty() {
            bail!("informe ao menos uma métrica");
        }
        if self.ids.as_ref().is_some_and(|ids| ids.trim().is_empty())
            || self.max_results == Some(0)
            || self.start_index == Some(0)
        {
            bail!("ids, max_results e start_index devem ser válidos");
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReportingListRequest {
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub include_system_managed: Option<bool>,
    pub on_behalf_of_content_owner: Option<String>,
}
impl ReportingListRequest {
    pub fn validate(&self) -> Result<()> {
        if self.page_size == Some(0) {
            bail!("page_size deve ser maior que zero");
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReportingJobRequest {
    pub report_type_id: String,
    pub name: String,
    pub on_behalf_of_content_owner: Option<String>,
}
impl ReportingJobRequest {
    pub fn validate(&self) -> Result<()> {
        validate_resource_id(&self.report_type_id)?;
        if self.name.trim().is_empty() {
            bail!("name é obrigatório");
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReportsListRequest {
    pub job_id: String,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    /// Timestamps opcionais em RFC 3339 UTC.
    pub created_after: Option<String>,
    pub start_time_at_or_after: Option<String>,
    pub start_time_before: Option<String>,
    pub on_behalf_of_content_owner: Option<String>,
}
impl ReportsListRequest {
    pub fn validate(&self) -> Result<()> {
        validate_resource_id(&self.job_id)?;
        if self.page_size == Some(0) {
            bail!("page_size deve ser maior que zero");
        }
        for time in [
            &self.created_after,
            &self.start_time_at_or_after,
            &self.start_time_before,
        ]
        .into_iter()
        .flatten()
        {
            chrono::DateTime::parse_from_rfc3339(time).context("timestamp deve usar RFC 3339")?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadReportRequest {
    pub job_id: String,
    pub report_id: String,
    /// Caminho absoluto do novo arquivo CSV; arquivos existentes não são sobrescritos.
    pub file_path: String,
    pub on_behalf_of_content_owner: Option<String>,
}
impl DownloadReportRequest {
    pub fn validate(&self) -> Result<()> {
        validate_resource_id(&self.job_id)?;
        validate_resource_id(&self.report_id)?;
        if !std::path::Path::new(&self.file_path).is_absolute() {
            bail!("file_path deve ser absoluto");
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CommentOrder {
    Time,
    Relevance,
}
impl CommentOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::Relevance => "relevance",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommentsRequest {
    pub video_id: String,
    /// Quantidade por página entre 1 e 100; padrão 20.
    pub max_results: Option<u8>,
    pub page_token: Option<String>,
    pub order: Option<CommentOrder>,
    /// Inclui uma amostra de respostas; use listar_respostas para obter todas as páginas.
    pub include_replies: Option<bool>,
}
impl CommentsRequest {
    pub fn validate(&self) -> Result<()> {
        validate_video_id(&self.video_id)?;
        validate_comment_page_size(self.max_results)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepliesRequest {
    /// ID do comentário principal.
    pub parent_id: String,
    pub max_results: Option<u8>,
    pub page_token: Option<String>,
}
impl RepliesRequest {
    pub fn validate(&self) -> Result<()> {
        validate_resource_id(&self.parent_id)?;
        validate_comment_page_size(self.max_results)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplyRequest {
    /// ID do comentário principal a responder.
    pub parent_id: String,
    pub text: String,
}
impl ReplyRequest {
    pub fn validate(&self) -> Result<()> {
        validate_resource_id(&self.parent_id)?;
        if self.text.trim().is_empty() {
            bail!("texto da resposta é obrigatório");
        }
        Ok(())
    }
}

fn validate_comment_page_size(size: Option<u8>) -> Result<()> {
    if size.is_some_and(|size| !(1..=100).contains(&size)) {
        bail!("max_results deve estar entre 1 e 100");
    }
    Ok(())
}

fn validate_resource_id(id: &str) -> Result<()> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        bail!("ID inválido");
    }
    Ok(())
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ShareResource {
    Video,
    Short,
    Playlist,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShareRequest {
    /// video, short ou playlist.
    pub resource: ShareResource,
    pub id: String,
}
impl ShareRequest {
    pub fn validate(&self) -> Result<()> {
        match self.resource {
            ShareResource::Video | ShareResource::Short => validate_video_id(&self.id),
            ShareResource::Playlist => validate_resource_id(&self.id),
        }
    }
    pub fn url(&self) -> String {
        match self.resource {
            ShareResource::Video => format!("https://www.youtube.com/watch?v={}", self.id),
            ShareResource::Short => format!("https://www.youtube.com/shorts/{}", self.id),
            ShareResource::Playlist => format!("https://www.youtube.com/playlist?list={}", self.id),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VideoIdRequest {
    /// ID do vídeo no YouTube.
    pub video_id: String,
}
impl VideoIdRequest {
    pub fn validate(&self) -> Result<()> {
        validate_video_id(&self.video_id)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ThumbnailRequest {
    /// ID do vídeo próprio no YouTube.
    pub video_id: String,
    /// Caminho absoluto da imagem JPEG ou PNG local.
    pub file_path: String,
}
impl ThumbnailRequest {
    pub fn validate(&self) -> Result<()> {
        validate_video_id(&self.video_id)?;
        if !std::path::Path::new(&self.file_path).is_absolute() {
            bail!("file_path deve ser absoluto");
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SyntheticMediaRequest {
    /// ID do vídeo do canal autenticado.
    pub video_id: String,
    /// Verdadeiro para declarar conteúdo realista alterado ou sintético.
    pub contains_synthetic_media: bool,
}
impl SyntheticMediaRequest {
    pub fn validate(&self) -> Result<()> {
        validate_video_id(&self.video_id)
    }
}

fn validate_video_id(id: &str) -> Result<()> {
    if id.len() != 11
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        bail!("video_id deve conter 11 caracteres válidos de ID do YouTube");
    }
    Ok(())
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadsRequest {
    /// Quantidade nesta página, entre 1 e 50; padrão 20.
    pub max_results: Option<u8>,
    /// Token de continuação retornado pela página anterior.
    pub page_token: Option<String>,
}
impl UploadsRequest {
    pub fn validate(&self) -> Result<()> {
        if self.max_results.is_some_and(|n| !(1..=50).contains(&n)) {
            bail!("max_results deve estar entre 1 e 50");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schedule_requires_private() {
        let mut v = VideoRequest {
            file_path: "/tmp/a.mp4".into(),
            title: "Teste".into(),
            description: None,
            privacy: Some(Privacy::Public),
            publish_at: Some("2026-12-01T22:00:00Z".into()),
            category_id: None,
            tags: None,
            made_for_kids: false,
            contains_synthetic_media: None,
            captions: None,
        };
        assert!(v.validate().is_err());
        v.privacy = Some(Privacy::Private);
        assert!(v.validate().is_ok());
    }
    #[test]
    fn video_privacy_defaults_to_public_except_when_scheduled() {
        let mut v = VideoRequest {
            file_path: "/tmp/video.mp4".into(),
            title: "Teste".into(),
            description: None,
            privacy: None,
            publish_at: None,
            category_id: None,
            tags: None,
            made_for_kids: false,
            contains_synthetic_media: None,
            captions: None,
        };
        assert_eq!(v.effective_privacy(), "public");
        v.privacy = Some(Privacy::Unlisted);
        assert_eq!(v.effective_privacy(), "unlisted");
        v.privacy = Some(Privacy::Private);
        assert_eq!(v.effective_privacy(), "private");
        v.privacy = None;
        v.publish_at = Some("2026-12-01T22:00:00Z".into());
        assert!(v.validate().is_ok());
        assert_eq!(v.effective_privacy(), "private");
    }

    #[test]
    fn caption_tracks_require_valid_paths_and_unique_language_name_pairs() {
        let mut video: VideoRequest = serde_json::from_value(serde_json::json!({
            "file_path":"/tmp/video.mp4", "title":"video", "made_for_kids":false,
            "captions":[{"file_path":"relative.srt", "language":"pt-BR", "name":"Português"}]
        }))
        .unwrap();
        assert!(video.validate().is_err());
        video.captions.as_mut().unwrap()[0].file_path = "/tmp/a.srt".into();
        assert!(video.validate().is_ok());
        video.captions.as_mut().unwrap().push(CaptionTrack {
            file_path: "/tmp/b.srt".into(),
            language: "pt-BR".into(),
            name: "Português".into(),
            is_draft: None,
        });
        assert!(video.validate().is_err());
    }

    #[test]
    fn analytics_validates_dates_and_pagination_without_restricting_metrics() {
        let mut request: AnalyticsRequest = serde_json::from_value(serde_json::json!({
            "start_date":"2026-01-01", "end_date":"2026-02-01", "metrics":"views,estimatedRevenue"
        }))
        .unwrap();
        assert!(request.validate().is_ok());
        request.start_index = Some(0);
        assert!(request.validate().is_err());
        request.start_index = Some(1);
        request.start_date = "2026-02-02".into();
        assert!(request.validate().is_err());
    }

    #[test]
    fn comment_and_statistics_input_boundaries_are_checked() {
        let request = CommentsRequest {
            video_id: "abcdefghijk".into(),
            max_results: Some(101),
            page_token: None,
            order: None,
            include_replies: None,
        };
        assert!(request.validate().is_err());
        let request = StatisticsRequest {
            resource: StatisticsResource::Video,
            ids: None,
        };
        assert!(request.validate().is_err());
        let request = StatisticsRequest {
            resource: StatisticsResource::Channel,
            ids: None,
        };
        assert!(request.validate().is_ok());
        let request = ReplyRequest {
            parent_id: "comment123".into(),
            text: " ".into(),
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn share_links_cover_video_short_and_playlist_without_injection() {
        let requests = [
            (
                ShareResource::Video,
                "abcdefghijk",
                "https://www.youtube.com/watch?v=abcdefghijk",
            ),
            (
                ShareResource::Short,
                "abcdefghijk",
                "https://www.youtube.com/shorts/abcdefghijk",
            ),
            (
                ShareResource::Playlist,
                "PL_123-abc",
                "https://www.youtube.com/playlist?list=PL_123-abc",
            ),
        ];
        for (resource, id, url) in requests {
            let request = ShareRequest {
                resource,
                id: id.into(),
            };
            assert!(request.validate().is_ok());
            assert_eq!(request.url(), url);
        }
        assert!(
            ShareRequest {
                resource: ShareResource::Playlist,
                id: "x&list=other".into()
            }
            .validate()
            .is_err()
        );
    }
}
