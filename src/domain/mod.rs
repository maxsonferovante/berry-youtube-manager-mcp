use anyhow::{Result, bail};
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
        Ok(())
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PostRequest {
    /// ID do canal autenticado.
    pub channel_id: String,
    /// Texto da postagem; opcional se houver imagem.
    pub text: Option<String>,
    /// Até 10 caminhos absolutos de imagens JPEG, PNG, GIF ou WebP.
    pub image_paths: Option<Vec<String>>,
}
impl PostRequest {
    pub fn validate(&self) -> Result<()> {
        if self.channel_id.trim().is_empty() {
            bail!("channel_id é obrigatório");
        }
        let images = self.image_paths.as_deref().unwrap_or_default();
        if images.len() > 10 {
            bail!("máximo de 10 imagens");
        }
        if images
            .iter()
            .any(|p| !std::path::Path::new(p).is_absolute())
        {
            bail!("os caminhos das imagens devem ser absolutos");
        }
        if images.is_empty() && self.text.as_deref().unwrap_or("").trim().is_empty() {
            bail!("informe texto ou imagem");
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
    fn post_requires_content_and_absolute_paths() {
        let mut p = PostRequest {
            channel_id: "UC123".into(),
            text: None,
            image_paths: None,
        };
        assert!(p.validate().is_err());
        p.image_paths = Some(vec!["relative.png".into()]);
        assert!(p.validate().is_err());
        p.image_paths = Some(vec!["/tmp/image.png".into()]);
        assert!(p.validate().is_ok());
    }
}
