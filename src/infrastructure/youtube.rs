use crate::{
    domain::{PostRequest, SyntheticMediaRequest, UploadsRequest, VideoIdRequest, VideoRequest},
    infrastructure::oauth,
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tokio_util::io::ReaderStream;

#[derive(Clone)]
pub struct YouTube {
    http: reqwest::Client,
}
impl YouTube {
    pub fn new() -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }
    async fn json(response: reqwest::Response) -> Result<Value> {
        let status = response.status();
        let value: Value = response
            .json()
            .await
            .context("resposta inválida da API do YouTube")?;
        if !status.is_success() {
            bail!(
                "YouTube retornou {status}: {}",
                value.get("error").unwrap_or(&value)
            );
        }
        Ok(value)
    }
}

impl YouTube {
    async fn channel_with_token(&self, token: &str) -> Result<Value> {
        Self::json(
            self.http
                .get("https://www.googleapis.com/youtube/v3/channels")
                .query(&[("part", "snippet,contentDetails"), ("mine", "true")])
                .bearer_auth(token)
                .send()
                .await?,
        )
        .await
    }
    async fn fetch_videos(&self, token: &str, ids: &str) -> Result<Value> {
        Self::json(
            self.http
                .get("https://www.googleapis.com/youtube/v3/videos")
                .query(&[
                    ("part", "snippet,status,processingDetails,contentDetails"),
                    ("id", ids),
                ])
                .bearer_auth(token)
                .send()
                .await?,
        )
        .await
    }
    async fn fetch_video(&self, token: &str, id: &str) -> Result<Value> {
        let mut response = self.fetch_videos(token, id).await?;
        let items = response
            .get_mut("items")
            .and_then(Value::as_array_mut)
            .context("resposta de vídeos sem items")?;
        if items.len() != 1 {
            bail!("vídeo não encontrado ou sem acesso: {id}");
        }
        Ok(items.remove(0))
    }
}

fn video_metadata(request: &VideoRequest) -> Value {
    let mut status = json!({"privacyStatus": request.effective_privacy(), "selfDeclaredMadeForKids": request.made_for_kids});
    if let Some(value) = request.contains_synthetic_media {
        status["containsSyntheticMedia"] = json!(value);
    }
    if let Some(date) = &request.publish_at {
        status["publishAt"] = json!(date);
    }
    json!({"snippet": {"title": request.title, "description": request.description.as_deref().unwrap_or_default(), "categoryId": request.category_id.as_deref().unwrap_or("10"), "tags": request.tags.as_deref().unwrap_or_default()}, "status":status})
}

// A porta isolada permite testar leitura, preservação e escrita sem tocar no YouTube.
trait StatusApi {
    async fn read_status(&self, token: &str, id: &str) -> Result<Value>;
    async fn write_status(&self, token: &str, body: Value, etag: &str) -> Result<Value>;
}
impl StatusApi for YouTube {
    async fn read_status(&self, token: &str, id: &str) -> Result<Value> {
        self.fetch_video(token, id).await
    }
    async fn write_status(&self, token: &str, body: Value, etag: &str) -> Result<Value> {
        let request = status_update_request(&self.http, token, &body, etag)?;
        let response = self
            .http
            .execute(request)
            .await
            .context("falha na atualização; confira o status no YouTube antes de repetir")?;
        if response.status() == reqwest::StatusCode::PRECONDITION_FAILED {
            bail!("o vídeo foi alterado desde a leitura; consulte novamente antes de atualizar");
        }
        Self::json(response).await
    }
}

fn status_update_request(
    http: &reqwest::Client,
    token: &str,
    body: &Value,
    etag: &str,
) -> Result<reqwest::Request> {
    Ok(http
        .put("https://www.googleapis.com/youtube/v3/videos")
        .query(&[("part", "status")])
        .bearer_auth(token)
        .header(reqwest::header::IF_MATCH, etag)
        .json(body)
        .build()?)
}

async fn update_synthetic_media(
    api: &impl StatusApi,
    token: &str,
    id: &str,
    value: bool,
) -> Result<Value> {
    let current = api.read_status(token, id).await?;
    let etag = current
        .get("etag")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .context(
            "vídeo sem ETag; atualização cancelada para não sobrescrever alterações concorrentes",
        )?;
    let body = status_update_body(&current, id, value)?;
    let response = api.write_status(token, body, etag).await?;
    Ok(
        json!({"video": response, "observacao": "Solicitação aceita pela API. Confira o status e o agendamento; a leitura pode não refletir a declaração de conteúdo sintético."}),
    )
}

fn status_update_body(current: &Value, id: &str, value: bool) -> Result<Value> {
    if current.get("id").and_then(Value::as_str) != Some(id) {
        bail!("a API retornou um vídeo diferente do solicitado");
    }
    let status = current
        .get("status")
        .and_then(Value::as_object)
        .context("vídeo sem status; atualização cancelada")?;
    let privacy = status
        .get("privacyStatus")
        .and_then(Value::as_str)
        .context("vídeo sem privacidade; atualização cancelada")?;
    if !["private", "public", "unlisted"].contains(&privacy) {
        bail!("privacidade desconhecida; atualização cancelada");
    }
    if status.get("license").and_then(Value::as_str).is_none()
        || status.get("embeddable").and_then(Value::as_bool).is_none()
        || status
            .get("publicStatsViewable")
            .and_then(Value::as_bool)
            .is_none()
    {
        bail!(
            "status incompleto; atualização cancelada para preservar as configurações existentes"
        );
    }
    if let Some(publish_at) = status.get("publishAt").filter(|v| !v.is_null()) {
        if privacy != "private" {
            bail!("agendamento inconsistente; atualização cancelada");
        }
        let date = publish_at.as_str().context("agendamento inválido")?;
        let date = chrono::DateTime::parse_from_rfc3339(date).context("agendamento inválido")?;
        if date <= chrono::Utc::now() + chrono::Duration::minutes(1) {
            bail!(
                "agendamento passou ou está a menos de um minuto; atualização cancelada para evitar publicação prematura"
            );
        }
    }
    let mut preserved = serde_json::Map::new();
    // Só propriedades mutáveis do status; campos somente leitura nunca são reenviados.
    for key in [
        "privacyStatus",
        "publishAt",
        "license",
        "embeddable",
        "publicStatsViewable",
        "selfDeclaredMadeForKids",
    ] {
        if let Some(v) = status.get(key).filter(|v| !v.is_null()) {
            preserved.insert(key.to_owned(), v.clone());
        }
    }
    preserved.insert("containsSyntheticMedia".into(), json!(value));
    Ok(json!({"id": id, "status": preserved}))
}

fn uploads_result(page: Value, videos: Value) -> Result<Value> {
    let playlist_items = page
        .get("items")
        .and_then(Value::as_array)
        .context("playlist sem items")?;
    let video_items = videos
        .get("items")
        .and_then(Value::as_array)
        .context("resposta de vídeos sem items")?;
    let items: Vec<Value> = playlist_items
        .iter()
        .map(|item| {
            let id = item
                .pointer("/contentDetails/videoId")
                .and_then(Value::as_str);
            let video = id.and_then(|id| {
                video_items
                    .iter()
                    .find(|v| v.get("id").and_then(Value::as_str) == Some(id))
            });
            json!({"playlist_item": item, "video": video})
        })
        .collect();
    Ok(
        json!({"items": items, "nextPageToken": page.get("nextPageToken"), "observacao": "Confira todas as páginas e os itens sem detalhes antes de concluir que um vídeo não existe; a listagem não impede uploads duplicados."}),
    )
}

impl crate::application::YouTubeGateway for YouTube {
    async fn channel(&self) -> Result<Value> {
        let token = oauth::access_token().await?;
        self.channel_with_token(&token).await
    }
    async fn get_video(&self, request: VideoIdRequest) -> Result<Value> {
        let token = oauth::access_token().await?;
        self.fetch_video(&token, &request.video_id).await
    }
    async fn list_uploads(&self, request: UploadsRequest) -> Result<Value> {
        let token = oauth::access_token().await?;
        let channel = self.channel_with_token(&token).await?;
        let playlist = channel
            .pointer("/items/0/contentDetails/relatedPlaylists/uploads")
            .and_then(Value::as_str)
            .context("a API não retornou a playlist de uploads do canal autenticado")?;
        let max = request.max_results.unwrap_or(20).to_string();
        let mut query = vec![
            ("part", "snippet,contentDetails"),
            ("playlistId", playlist),
            ("maxResults", max.as_str()),
        ];
        if let Some(page_token) = request.page_token.as_deref() {
            query.push(("pageToken", page_token));
        }
        let page = Self::json(
            self.http
                .get("https://www.googleapis.com/youtube/v3/playlistItems")
                .query(&query)
                .bearer_auth(&token)
                .send()
                .await?,
        )
        .await?;
        let ids: Vec<&str> = page
            .get("items")
            .and_then(Value::as_array)
            .context("resposta da playlist sem items")?
            .iter()
            .filter_map(|item| {
                item.pointer("/contentDetails/videoId")
                    .and_then(Value::as_str)
            })
            .collect();
        let videos = if ids.is_empty() {
            json!({"items": []})
        } else {
            self.fetch_videos(&token, &ids.join(",")).await?
        };
        uploads_result(page, videos)
    }
    async fn set_synthetic_media(&self, request: SyntheticMediaRequest) -> Result<Value> {
        let token = oauth::access_token().await?;
        update_synthetic_media(
            self,
            &token,
            &request.video_id,
            request.contains_synthetic_media,
        )
        .await
    }
    async fn upload_video(&self, request: VideoRequest) -> Result<Value> {
        let file = tokio::fs::File::open(&request.file_path)
            .await
            .context("não foi possível abrir o vídeo")?;
        let size = file.metadata().await?.len();
        if size == 0 {
            bail!("o vídeo está vazio");
        }
        let token = oauth::access_token().await?;
        let metadata = video_metadata(&request);
        let init = self
            .http
            .post("https://www.googleapis.com/upload/youtube/v3/videos")
            .query(&[("uploadType", "resumable"), ("part", "snippet,status")])
            .bearer_auth(&token)
            .header("X-Upload-Content-Length", size)
            .header("X-Upload-Content-Type", "video/*")
            .json(&metadata)
            .send()
            .await?;
        if !init.status().is_success() {
            return Self::json(init).await;
        }
        let location = init
            .headers()
            .get(reqwest::header::LOCATION)
            .context("YouTube não retornou endereço de upload")?
            .to_str()?;
        let url = url::Url::parse(location)?;
        if url.scheme() != "https" || url.host_str() != Some("www.googleapis.com") {
            bail!("endereço de upload inesperado");
        }
        let response = self.http.put(url).bearer_auth(token).header(reqwest::header::CONTENT_TYPE, "video/*").header(reqwest::header::CONTENT_LENGTH, size).body(reqwest::Body::wrap_stream(ReaderStream::new(file))).send().await.context("falha no envio; confira o YouTube Studio antes de tentar novamente, pois o resultado pode ser incerto")?;
        if response.status().as_u16() == 308 {
            bail!("upload incompleto; confira o YouTube Studio antes de tentar novamente");
        }
        let mut uploaded = Self::json(response).await?;
        if let Some(object) = uploaded.as_object_mut() {
            object.insert("mcp_pendencias".into(), json!({
                "video_relacionado": "A API Data v3 não documenta a associação de vídeo relacionado de Short; configure no YouTube Studio."
            }));
        }
        Ok(uploaded)
    }
    async fn publish_post(&self, request: PostRequest) -> Result<Value> {
        let token = oauth::access_token().await?;
        let mut images = Vec::new();
        for path in request.image_paths.unwrap_or_default() {
            let bytes = tokio::fs::read(path)
                .await
                .context("não foi possível abrir a imagem")?;
            if bytes.is_empty() || bytes.len() > 16_000_000 {
                bail!("cada imagem deve ter até 16 MB e não pode estar vazia");
            }
            let response = self
                .http
                .post("https://www.googleapis.com/upload/youtube/v3/posts/image")
                .query(&[
                    ("channelId", &request.channel_id),
                    ("uploadType", &"media".to_string()),
                ])
                .bearer_auth(&token)
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .body(bytes)
                .send()
                .await?;
            let value = Self::json(response).await?;
            let image_token = value
                .get("imageUploadToken")
                .and_then(Value::as_str)
                .context("YouTube não retornou token da imagem")?;
            images.push(json!({"uploadToken": image_token}));
        }
        let mut body = json!({"snippet": {"channelId": request.channel_id, "textDisplay":request.text.unwrap_or_default()}});
        let part = if images.is_empty() {
            "snippet,id,status"
        } else {
            body["attachments"] = json!({"imageAttachments": images});
            "snippet,attachments,id,status"
        };
        let response = self.http.post("https://www.googleapis.com/youtube/v3/posts").query(&[("part", part)]).bearer_auth(token).json(&body).send().await.context("falha ao publicar; confira o YouTube antes de tentar novamente, pois o resultado pode ser incerto")?;
        Self::json(response).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockStatusApi {
        current: Value,
        writes: Arc<Mutex<Vec<(Value, String)>>>,
    }
    impl StatusApi for MockStatusApi {
        async fn read_status(&self, _: &str, _: &str) -> Result<Value> {
            Ok(self.current.clone())
        }
        async fn write_status(&self, _: &str, body: Value, etag: &str) -> Result<Value> {
            self.writes
                .lock()
                .expect("mock lock")
                .push((body.clone(), etag.to_owned()));
            Ok(body)
        }
    }
    fn sample_request(value: Option<bool>) -> VideoRequest {
        VideoRequest {
            file_path: "/tmp/a.mp4".into(),
            title: "Teste".into(),
            description: None,
            privacy: None,
            publish_at: None,
            category_id: None,
            tags: None,
            made_for_kids: false,
            contains_synthetic_media: value,
        }
    }
    #[test]
    fn upload_declares_true_false_or_omits_unknown() {
        assert_eq!(
            video_metadata(&sample_request(Some(true)))["status"]["containsSyntheticMedia"],
            true
        );
        assert_eq!(
            video_metadata(&sample_request(Some(false)))["status"]["containsSyntheticMedia"],
            false
        );
        assert!(
            video_metadata(&sample_request(None))["status"]
                .get("containsSyntheticMedia")
                .is_none()
        );
    }
    #[tokio::test]
    async fn update_reads_status_preserves_mutable_fields_and_uses_etag() {
        let api = MockStatusApi {
            current: json!({"id":"abcdefghijk", "etag":"\"v1\"", "status":{"privacyStatus":"private", "publishAt":"2099-11-25T22:00:00Z", "license":"youtube", "embeddable":true, "publicStatsViewable":false, "selfDeclaredMadeForKids":false, "uploadStatus":"processed", "madeForKids":false}}),
            writes: Arc::default(),
        };
        update_synthetic_media(&api, "fake", "abcdefghijk", true)
            .await
            .expect("update");
        let writes = api.writes.lock().expect("mock lock");
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].1, "\"v1\"");
        assert_eq!(
            writes[0].0,
            json!({"id":"abcdefghijk", "status":{"privacyStatus":"private", "publishAt":"2099-11-25T22:00:00Z", "license":"youtube", "embeddable":true, "publicStatsViewable":false, "selfDeclaredMadeForKids":false, "containsSyntheticMedia":true}})
        );
    }
    #[test]
    fn update_request_sends_status_and_conditional_etag() {
        let body = json!({"id":"abcdefghijk", "status":{"privacyStatus":"private", "containsSyntheticMedia":true}});
        let req = status_update_request(&reqwest::Client::new(), "fake", &body, "\"v1\"")
            .expect("request");
        assert_eq!(req.method(), reqwest::Method::PUT);
        assert_eq!(req.url().path(), "/youtube/v3/videos");
        assert!(
            req.url()
                .query_pairs()
                .any(|(key, value)| key == "part" && value == "status")
        );
        assert_eq!(req.headers()[reqwest::header::IF_MATCH], "\"v1\"");
        assert_eq!(
            serde_json::from_slice::<Value>(
                req.body().and_then(|b| b.as_bytes()).expect("json body")
            )
            .expect("json"),
            body
        );
    }
    #[tokio::test]
    async fn update_aborts_without_etag_or_with_expired_schedule() {
        let writes = Arc::default();
        let mut api = MockStatusApi {
            current: json!({"id":"abcdefghijk", "status":{"privacyStatus":"private"}}),
            writes,
        };
        assert!(
            update_synthetic_media(&api, "fake", "abcdefghijk", true)
                .await
                .is_err()
        );
        api.current = json!({"id":"abcdefghijk", "etag":"v1", "status":{"privacyStatus":"private", "license":"youtube", "embeddable":true, "publicStatsViewable":true, "publishAt":"2000-01-01T00:00:00Z"}});
        assert!(
            update_synthetic_media(&api, "fake", "abcdefghijk", true)
                .await
                .is_err()
        );
        assert!(api.writes.lock().expect("mock lock").is_empty());
    }
    #[test]
    fn uploads_keep_order_pagination_and_unavailable_items() {
        let page = json!({"nextPageToken":"next", "items":[{"contentDetails":{"videoId":"abcdefghijk"}}, {"contentDetails":{"videoId":"zyxwvutsrqp"}}]});
        let videos = json!({"items":[{"id":"abcdefghijk", "status":{"privacyStatus":"private"}, "processingDetails":{"processingStatus":"succeeded"}}]});
        let result = uploads_result(page, videos).expect("page");
        assert_eq!(result["nextPageToken"], "next");
        assert_eq!(
            result["items"][0]["video"]["status"]["privacyStatus"],
            "private"
        );
        assert!(result["items"][1]["video"].is_null());
        assert_eq!(
            result["items"][1]["playlist_item"]["contentDetails"]["videoId"],
            "zyxwvutsrqp"
        );
    }
}
