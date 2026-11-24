use super::YouTube;
use crate::domain::{
    AnalyticsRequest, CaptionTrack, CommentsRequest, DownloadReportRequest, RepliesRequest,
    ReplyRequest, ReportingJobRequest, ReportingListRequest, ReportsListRequest, StatisticsRequest,
    StatisticsResource,
};
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{Client, Request};
use serde_json::{Value, json};

const DATA_API: &str = "https://www.googleapis.com/youtube/v3";
const ANALYTICS_API: &str = "https://youtubeanalytics.googleapis.com/v2/reports";
const CAPTIONS_API: &str = "https://www.googleapis.com/upload/youtube/v3/captions";
const REPORTING_API: &str = "https://youtubereporting.googleapis.com/v1";
const MAX_CAPTION_BYTES: u64 = 100_000_000;

pub(super) struct PreparedCaption {
    track: CaptionTrack,
    bytes: Vec<u8>,
}

pub(super) async fn prepare_captions(tracks: Vec<CaptionTrack>) -> Result<Vec<PreparedCaption>> {
    let mut prepared = Vec::with_capacity(tracks.len());
    let mut total_bytes = 0_u64;
    for track in tracks {
        let file = tokio::fs::File::open(&track.file_path)
            .await
            .with_context(|| format!("não foi possível abrir a legenda {}", track.file_path))?;
        let size = file.metadata().await?.len();
        if size == 0 || size > MAX_CAPTION_BYTES - total_bytes {
            bail!("as legendas devem conter até 100 MB no total e não podem estar vazias");
        }
        use tokio::io::AsyncReadExt;
        let mut bytes = Vec::with_capacity(size as usize);
        file.take(MAX_CAPTION_BYTES - total_bytes + 1)
            .read_to_end(&mut bytes)
            .await?;
        if bytes.is_empty() || bytes.len() as u64 > MAX_CAPTION_BYTES - total_bytes {
            bail!("as legendas devem conter até 100 MB no total e não podem estar vazias");
        }
        total_bytes += bytes.len() as u64;
        prepared.push(PreparedCaption { track, bytes });
    }
    Ok(prepared)
}

fn caption_request(
    http: &Client,
    token: &str,
    video_id: &str,
    caption: &PreparedCaption,
    boundary: &str,
) -> Result<Request> {
    let metadata = json!({"snippet": {
        "videoId": video_id,
        "language": caption.track.language,
        "name": caption.track.name,
        "isDraft": caption.track.is_draft.unwrap_or(false)
    }});
    let mut body = format!(
        "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n"
    ).into_bytes();
    body.extend_from_slice(&caption.bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Ok(http
        .post(CAPTIONS_API)
        .query(&[("part", "snippet"), ("uploadType", "multipart")])
        .bearer_auth(token)
        .header(
            reqwest::header::CONTENT_TYPE,
            format!("multipart/related; boundary={boundary}"),
        )
        .body(body)
        .build()?)
}

pub(super) trait CaptionApi {
    async fn send_caption(
        &self,
        token: &str,
        video_id: &str,
        caption: &PreparedCaption,
    ) -> Result<Value>;
}
impl CaptionApi for YouTube {
    async fn send_caption(
        &self,
        token: &str,
        video_id: &str,
        caption: &PreparedCaption,
    ) -> Result<Value> {
        let mut random = [0_u8; 24];
        getrandom::fill(&mut random)
            .map_err(|_| anyhow::anyhow!("não foi possível gerar separador seguro"))?;
        let boundary = format!("berry-{}", URL_SAFE_NO_PAD.encode(random));
        let request = caption_request(&self.http, token, video_id, caption, &boundary)?;
        let response = self.http.execute(request).await.context(
            "falha ao enviar legenda; confira as legendas do vídeo antes de tentar novamente",
        )?;
        Self::json(response).await
    }
}

// O vídeo já existe aqui: conserve seu ID mesmo se alguma faixa não for enviada.
pub(super) async fn attach_uploaded_captions(
    api: &impl CaptionApi,
    token: &str,
    video: &mut Value,
    captions: Vec<PreparedCaption>,
) {
    if captions.is_empty() {
        return;
    }
    if !video.is_object() {
        *video = json!({"resposta_upload": video.take()});
    }
    let video_id = video.get("id").and_then(Value::as_str).map(str::to_owned);
    let mut results = Vec::new();
    for caption in captions {
        let result = if let Some(id) = &video_id {
            match api.send_caption(token, id, &caption).await {
                Ok(value) => json!({"status":"enviada", "resposta":value}),
                Err(error) => json!({"status":"falhou", "erro":format!("{error:#}")}),
            }
        } else {
            json!({"status":"falhou", "erro":"YouTube não retornou ID do vídeo; consulte o canal antes de tentar novamente"})
        };
        results.push(json!({
            "file_path":caption.track.file_path,
            "language":caption.track.language,
            "name":caption.track.name,
            "resultado":result
        }));
    }
    video["mcp_legendas"] = json!(results);
}

fn statistics_request(http: &Client, token: &str, request: &StatisticsRequest) -> Result<Request> {
    let (path, part, filter) = match request.resource {
        StatisticsResource::Video => ("videos", "snippet,contentDetails,statistics", "id"),
        StatisticsResource::Channel => ("channels", "snippet,contentDetails,statistics", "id"),
        StatisticsResource::Playlist => ("playlists", "snippet,contentDetails,status", "id"),
        StatisticsResource::Post => ("posts", "snippet,statistics,status,attachments", "postId"),
    };
    let mut params = vec![("part", part.to_owned())];
    if let Some(ids) = &request.ids {
        params.push((filter, ids.join(",")));
    } else {
        params.push(("mine", "true".to_owned()));
    }
    Ok(http
        .get(format!("{DATA_API}/{path}"))
        .query(&params)
        .bearer_auth(token)
        .build()?)
}

fn analytics_request(http: &Client, token: &str, request: &AnalyticsRequest) -> Result<Request> {
    let mut params = vec![
        (
            "ids",
            request.ids.as_deref().unwrap_or("channel==MINE").to_owned(),
        ),
        ("startDate", request.start_date.clone()),
        ("endDate", request.end_date.clone()),
        ("metrics", request.metrics.clone()),
    ];
    for (key, value) in [
        ("dimensions", request.dimensions.as_ref()),
        ("filters", request.filters.as_ref()),
        ("sort", request.sort.as_ref()),
        ("currency", request.currency.as_ref()),
    ] {
        if let Some(value) = value {
            params.push((key, value.clone()));
        }
    }
    if let Some(value) = request.max_results {
        params.push(("maxResults", value.to_string()));
    }
    if let Some(value) = request.start_index {
        params.push(("startIndex", value.to_string()));
    }
    if let Some(value) = request.include_historical_channel_data {
        params.push(("includeHistoricalChannelData", value.to_string()));
    }
    Ok(http
        .get(ANALYTICS_API)
        .query(&params)
        .bearer_auth(token)
        .build()?)
}

fn comments_request(http: &Client, token: &str, request: &CommentsRequest) -> Result<Request> {
    let mut params = vec![
        (
            "part",
            if request.include_replies.unwrap_or(false) {
                "snippet,replies".to_owned()
            } else {
                "snippet".to_owned()
            },
        ),
        ("videoId", request.video_id.clone()),
        ("maxResults", request.max_results.unwrap_or(20).to_string()),
        ("textFormat", "plainText".to_owned()),
    ];
    if let Some(order) = &request.order {
        params.push(("order", order.as_str().to_owned()));
    }
    if let Some(page) = &request.page_token {
        params.push(("pageToken", page.clone()));
    }
    Ok(http
        .get(format!("{DATA_API}/commentThreads"))
        .query(&params)
        .bearer_auth(token)
        .build()?)
}

fn replies_request(http: &Client, token: &str, request: &RepliesRequest) -> Result<Request> {
    let mut params = vec![
        ("part", "snippet".to_owned()),
        ("parentId", request.parent_id.clone()),
        ("maxResults", request.max_results.unwrap_or(20).to_string()),
        ("textFormat", "plainText".to_owned()),
    ];
    if let Some(page) = &request.page_token {
        params.push(("pageToken", page.clone()));
    }
    Ok(http
        .get(format!("{DATA_API}/comments"))
        .query(&params)
        .bearer_auth(token)
        .build()?)
}

fn reply_request(http: &Client, token: &str, request: &ReplyRequest) -> Result<Request> {
    Ok(http
        .post(format!("{DATA_API}/comments"))
        .query(&[("part", "snippet")])
        .bearer_auth(token)
        .json(&json!({"snippet":{"parentId":request.parent_id,"textOriginal":request.text}}))
        .build()?)
}

fn reporting_list_request(
    http: &Client,
    token: &str,
    resource: &str,
    request: &ReportingListRequest,
) -> Result<Request> {
    let mut params = Vec::new();
    if let Some(value) = request.page_size {
        params.push(("pageSize", value.to_string()));
    }
    if let Some(value) = &request.page_token {
        params.push(("pageToken", value.clone()));
    }
    if let Some(value) = request.include_system_managed {
        params.push(("includeSystemManaged", value.to_string()));
    }
    if let Some(value) = &request.on_behalf_of_content_owner {
        params.push(("onBehalfOfContentOwner", value.clone()));
    }
    Ok(http
        .get(format!("{REPORTING_API}/{resource}"))
        .query(&params)
        .bearer_auth(token)
        .build()?)
}

fn reporting_job_request(
    http: &Client,
    token: &str,
    request: &ReportingJobRequest,
) -> Result<Request> {
    let mut builder = http
        .post(format!("{REPORTING_API}/jobs"))
        .bearer_auth(token)
        .json(&json!({"reportTypeId": request.report_type_id, "name": request.name}));
    if let Some(owner) = &request.on_behalf_of_content_owner {
        builder = builder.query(&[("onBehalfOfContentOwner", owner)]);
    }
    Ok(builder.build()?)
}

fn reports_list_request(
    http: &Client,
    token: &str,
    request: &ReportsListRequest,
) -> Result<Request> {
    let mut params = Vec::new();
    if let Some(value) = request.page_size {
        params.push(("pageSize", value.to_string()));
    }
    for (key, value) in [
        ("pageToken", &request.page_token),
        ("createdAfter", &request.created_after),
        ("startTimeAtOrAfter", &request.start_time_at_or_after),
        ("startTimeBefore", &request.start_time_before),
        (
            "onBehalfOfContentOwner",
            &request.on_behalf_of_content_owner,
        ),
    ] {
        if let Some(value) = value {
            params.push((key, value.clone()));
        }
    }
    Ok(http
        .get(format!("{REPORTING_API}/jobs/{}/reports", request.job_id))
        .query(&params)
        .bearer_auth(token)
        .build()?)
}

fn report_metadata_request(
    http: &Client,
    token: &str,
    request: &DownloadReportRequest,
) -> Result<Request> {
    let mut builder = http
        .get(format!(
            "{REPORTING_API}/jobs/{}/reports/{}",
            request.job_id, request.report_id
        ))
        .bearer_auth(token);
    if let Some(owner) = &request.on_behalf_of_content_owner {
        builder = builder.query(&[("onBehalfOfContentOwner", owner)]);
    }
    Ok(builder.build()?)
}

fn validated_report_url(value: &str) -> Result<url::Url> {
    let url = url::Url::parse(value)?;
    if url.scheme() != "https"
        || url.host_str() != Some("youtubereporting.googleapis.com")
        || !url.path().starts_with("/v1/media/")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|port| port != 443)
    {
        bail!("endereço de relatório inesperado; download cancelado");
    }
    Ok(url)
}

async fn save_report(mut response: reqwest::Response, path: &str) -> Result<u64> {
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await
        .context("não foi possível criar o arquivo; um arquivo existente nunca é sobrescrito")?;
    let write_result: Result<u64> = async {
        let mut bytes = 0_u64;
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
            bytes += chunk.len() as u64;
        }
        file.flush().await?;
        Ok(bytes)
    }
    .await;
    drop(file);
    match write_result {
        Ok(bytes) => Ok(bytes),
        Err(error) => {
            if let Err(cleanup) = tokio::fs::remove_file(path).await {
                return Err(error.context(format!(
                    "falha no download e ao remover o arquivo parcial: {cleanup}"
                )));
            }
            Err(error.context("falha no download; arquivo parcial removido"))
        }
    }
}

impl YouTube {
    pub(super) async fn reporting_list_with_token(
        &self,
        token: &str,
        resource: &str,
        request: &ReportingListRequest,
    ) -> Result<Value> {
        Self::json(
            self.http
                .execute(reporting_list_request(
                    &self.http, token, resource, request,
                )?)
                .await?,
        )
        .await
    }
    pub(super) async fn create_reporting_job_with_token(
        &self,
        token: &str,
        request: &ReportingJobRequest,
    ) -> Result<Value> {
        let response = self
            .http
            .execute(reporting_job_request(&self.http, token, request)?)
            .await
            .context("falha ao criar job; confira os jobs existentes antes de repetir")?;
        Self::json(response).await
    }
    pub(super) async fn reports_with_token(
        &self,
        token: &str,
        request: &ReportsListRequest,
    ) -> Result<Value> {
        Self::json(
            self.http
                .execute(reports_list_request(&self.http, token, request)?)
                .await?,
        )
        .await
    }
    pub(super) async fn download_report_with_token(
        &self,
        token: &str,
        request: &DownloadReportRequest,
    ) -> Result<Value> {
        let metadata = Self::json(
            self.http
                .execute(report_metadata_request(&self.http, token, request)?)
                .await?,
        )
        .await?;
        let url = validated_report_url(
            metadata
                .get("downloadUrl")
                .and_then(Value::as_str)
                .context("relatório sem downloadUrl")?,
        )?;
        let response = self.http.get(url).bearer_auth(token).send().await?;
        if !response.status().is_success() {
            return Self::json(response).await;
        }
        let bytes = save_report(response, &request.file_path).await?;
        Ok(json!({"file_path": request.file_path, "bytes": bytes, "report": metadata}))
    }
    pub(super) async fn statistics_with_token(
        &self,
        token: &str,
        request: &StatisticsRequest,
    ) -> Result<Value> {
        Self::json(
            self.http
                .execute(statistics_request(&self.http, token, request)?)
                .await?,
        )
        .await
    }
    pub(super) async fn analytics_with_token(
        &self,
        token: &str,
        request: &AnalyticsRequest,
    ) -> Result<Value> {
        Self::json(
            self.http
                .execute(analytics_request(&self.http, token, request)?)
                .await?,
        )
        .await
    }
    pub(super) async fn comments_with_token(
        &self,
        token: &str,
        request: &CommentsRequest,
    ) -> Result<Value> {
        Self::json(
            self.http
                .execute(comments_request(&self.http, token, request)?)
                .await?,
        )
        .await
    }
    pub(super) async fn replies_with_token(
        &self,
        token: &str,
        request: &RepliesRequest,
    ) -> Result<Value> {
        Self::json(
            self.http
                .execute(replies_request(&self.http, token, request)?)
                .await?,
        )
        .await
    }
    pub(super) async fn reply_with_token(
        &self,
        token: &str,
        request: &ReplyRequest,
    ) -> Result<Value> {
        let response = self
            .http
            .execute(reply_request(&self.http, token, request)?)
            .await
            .context(
                "falha ao responder; confira o comentário no YouTube antes de tentar novamente",
            )?;
        Self::json(response).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CommentOrder, StatisticsResource};
    use std::sync::{Arc, Mutex};

    fn track(language: &str, name: &str) -> CaptionTrack {
        CaptionTrack {
            file_path: "/tmp/captions.srt".into(),
            language: language.into(),
            name: name.into(),
            is_draft: Some(true),
        }
    }
    fn prepared(language: &str, name: &str) -> PreparedCaption {
        PreparedCaption {
            track: track(language, name),
            bytes: b"1\n00:00:01,000 --> 00:00:02,000\nHello!\n".to_vec(),
        }
    }
    fn query(request: &Request) -> std::collections::HashMap<String, String> {
        request.url().query_pairs().into_owned().collect()
    }

    #[test]
    fn caption_upload_uses_youtube_multipart_contract() {
        let request = caption_request(
            &Client::new(),
            "token",
            "abcdefghijk",
            &prepared("pt-BR", "Português"),
            "boundary123",
        )
        .unwrap();
        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(request.url().path(), "/upload/youtube/v3/captions");
        assert_eq!(query(&request)["uploadType"], "multipart");
        assert_eq!(
            request.headers()[reqwest::header::CONTENT_TYPE],
            "multipart/related; boundary=boundary123"
        );
        let body = std::str::from_utf8(request.body().unwrap().as_bytes().unwrap()).unwrap();
        assert!(body.contains("\"videoId\":\"abcdefghijk\""));
        assert!(body.contains("\"language\":\"pt-BR\""));
        assert!(body.contains("\"isDraft\":true"));
        assert!(body.contains("00:00:01,000 --> 00:00:02,000"));
        assert!(body.ends_with("\r\n--boundary123--\r\n"));
    }

    #[derive(Default)]
    struct FakeCaptionApi(Arc<Mutex<Vec<String>>>);
    impl CaptionApi for FakeCaptionApi {
        async fn send_caption(
            &self,
            _: &str,
            video_id: &str,
            caption: &PreparedCaption,
        ) -> Result<Value> {
            self.0
                .lock()
                .unwrap()
                .push(format!("{video_id}:{}", caption.track.language));
            if caption.track.language == "en" {
                bail!("falha simulada");
            }
            Ok(json!({"id":"caption123"}))
        }
    }

    #[tokio::test]
    async fn partial_caption_failure_preserves_video_id_and_other_results() {
        let api = FakeCaptionApi::default();
        let mut video = json!({"id":"abcdefghijk", "status":{"privacyStatus":"private"}});
        attach_uploaded_captions(
            &api,
            "token",
            &mut video,
            vec![prepared("en", "English"), prepared("pt-BR", "Português")],
        )
        .await;
        assert_eq!(video["id"], "abcdefghijk");
        assert_eq!(video["status"]["privacyStatus"], "private");
        assert_eq!(video["mcp_legendas"][0]["resultado"]["status"], "falhou");
        assert_eq!(
            video["mcp_legendas"][1]["resultado"]["resposta"]["id"],
            "caption123"
        );
        assert_eq!(api.0.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn missing_video_id_or_caption_file_does_not_send_caption() {
        let api = FakeCaptionApi::default();
        let mut video = json!({"unexpected":"response"});
        attach_uploaded_captions(&api, "token", &mut video, vec![prepared("en", "English")]).await;
        assert_eq!(video["mcp_legendas"][0]["resultado"]["status"], "falhou");
        assert!(api.0.lock().unwrap().is_empty());
        let mut missing = track("en", "English");
        missing.file_path = "/this/file/cannot/exist/berry.srt".into();
        assert!(prepare_captions(vec![missing]).await.is_err());
    }

    #[test]
    fn analytics_forwards_filters_pagination_and_returns_raw_report_contract() {
        let request = AnalyticsRequest {
            ids: None,
            start_date: "2026-01-01".into(),
            end_date: "2026-02-01".into(),
            metrics: "views,estimatedMinutesWatched".into(),
            dimensions: Some("day,country".into()),
            filters: Some("video==abcdefghijk;country==BR".into()),
            sort: Some("-views".into()),
            currency: Some("BRL".into()),
            max_results: Some(25),
            start_index: Some(26),
            include_historical_channel_data: Some(false),
        };
        let http = analytics_request(&Client::new(), "token", &request).unwrap();
        let q = query(&http);
        assert_eq!(
            http.url().host_str(),
            Some("youtubeanalytics.googleapis.com")
        );
        assert_eq!(q["ids"], "channel==MINE");
        assert_eq!(q["filters"], "video==abcdefghijk;country==BR");
        assert_eq!(q["dimensions"], "day,country");
        assert_eq!(q["maxResults"], "25");
        assert_eq!(q["startIndex"], "26");
        assert_eq!(q["includeHistoricalChannelData"], "false");
    }

    #[test]
    fn statistics_choose_real_resource_and_filter() {
        let channel = StatisticsRequest {
            resource: StatisticsResource::Channel,
            ids: None,
        };
        let http = statistics_request(&Client::new(), "token", &channel).unwrap();
        assert_eq!(http.url().path(), "/youtube/v3/channels");
        assert_eq!(query(&http)["mine"], "true");
        assert!(query(&http)["part"].contains("statistics"));
        let post = StatisticsRequest {
            resource: StatisticsResource::Post,
            ids: Some(vec!["post123".into()]),
        };
        let http = statistics_request(&Client::new(), "token", &post).unwrap();
        assert_eq!(http.url().path(), "/youtube/v3/posts");
        assert_eq!(query(&http)["postId"], "post123");
        let playlist = StatisticsRequest {
            resource: StatisticsResource::Playlist,
            ids: Some(vec!["PL123".into()]),
        };
        let http = statistics_request(&Client::new(), "token", &playlist).unwrap();
        assert!(query(&http)["part"].contains("contentDetails"));
        assert!(!query(&http)["part"].contains("statistics"));
    }

    #[test]
    fn comments_and_replies_use_pagination_and_correct_write_payload() {
        let comments = CommentsRequest {
            video_id: "abcdefghijk".into(),
            max_results: Some(100),
            page_token: Some("next".into()),
            order: Some(CommentOrder::Time),
            include_replies: Some(true),
        };
        let http = comments_request(&Client::new(), "token", &comments).unwrap();
        assert_eq!(http.url().path(), "/youtube/v3/commentThreads");
        assert_eq!(query(&http)["part"], "snippet,replies");
        assert_eq!(query(&http)["pageToken"], "next");
        let replies = RepliesRequest {
            parent_id: "comment123".into(),
            max_results: Some(25),
            page_token: Some("next2".into()),
        };
        let http = replies_request(&Client::new(), "token", &replies).unwrap();
        assert_eq!(query(&http)["parentId"], "comment123");
        assert_eq!(query(&http)["pageToken"], "next2");
        let reply = ReplyRequest {
            parent_id: "comment123".into(),
            text: "Obrigado!".into(),
        };
        let http = reply_request(&Client::new(), "token", &reply).unwrap();
        assert_eq!(http.method(), reqwest::Method::POST);
        let body: Value = serde_json::from_slice(http.body().unwrap().as_bytes().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({"snippet":{"parentId":"comment123","textOriginal":"Obrigado!"}})
        );
    }

    #[test]
    fn reporting_requests_support_system_managed_content_owner_and_filters() {
        let options = ReportingListRequest {
            page_size: Some(50),
            page_token: Some("next".into()),
            include_system_managed: Some(true),
            on_behalf_of_content_owner: Some("owner".into()),
        };
        let http =
            reporting_list_request(&Client::new(), "token", "reportTypes", &options).unwrap();
        assert_eq!(http.url().path(), "/v1/reportTypes");
        assert_eq!(query(&http)["includeSystemManaged"], "true");
        assert_eq!(query(&http)["onBehalfOfContentOwner"], "owner");
        let job = ReportingJobRequest {
            report_type_id: "channel_basic_a3".into(),
            name: "Daily".into(),
            on_behalf_of_content_owner: None,
        };
        let http = reporting_job_request(&Client::new(), "token", &job).unwrap();
        assert_eq!(http.method(), reqwest::Method::POST);
        let body: Value = serde_json::from_slice(http.body().unwrap().as_bytes().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({"name":"Daily", "reportTypeId":"channel_basic_a3"})
        );
        let reports = ReportsListRequest {
            job_id: "job-123".into(),
            page_size: None,
            page_token: Some("next".into()),
            created_after: None,
            start_time_at_or_after: Some("2026-01-01T00:00:00Z".into()),
            start_time_before: None,
            on_behalf_of_content_owner: None,
        };
        let http = reports_list_request(&Client::new(), "token", &reports).unwrap();
        assert_eq!(http.url().path(), "/v1/jobs/job-123/reports");
        assert_eq!(query(&http)["startTimeAtOrAfter"], "2026-01-01T00:00:00Z");
    }

    #[test]
    fn report_download_never_sends_token_to_unexpected_hosts() {
        assert!(validated_report_url("https://youtubereporting.googleapis.com/v1/media/CHANNEL/id/jobs/job/reports/report?alt=media").is_ok());
        assert!(
            validated_report_url(
                "https://youtubereporting.googleapis.com.evil.example/v1/media/foo"
            )
            .is_err()
        );
        assert!(
            validated_report_url("http://youtubereporting.googleapis.com/v1/media/foo").is_err()
        );
        assert!(
            validated_report_url("https://youtubereporting.googleapis.com/v1/jobs/foo").is_err()
        );
    }
    async fn fake_response(declared_length: usize, body: &'static str) -> reqwest::Response {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer).await.unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {declared_length}\r\nConnection: close\r\n\r\n{body}"
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        Client::new()
            .get(format!("http://{address}/report"))
            .send()
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn bulk_csv_is_saved_verbatim_without_overwrite_and_partial_file_is_removed() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("berry-report-{}-{unique}.csv", std::process::id()));
        let path = path.to_str().unwrap();
        let csv = "day,views\n2026-01-01,42\n";
        let response = fake_response(csv.len(), csv).await;
        assert_eq!(save_report(response, path).await.unwrap(), csv.len() as u64);
        assert_eq!(tokio::fs::read_to_string(path).await.unwrap(), csv);
        let response = fake_response(csv.len(), csv).await;
        assert!(save_report(response, path).await.is_err());
        assert_eq!(tokio::fs::read_to_string(path).await.unwrap(), csv);
        tokio::fs::remove_file(path).await.unwrap();
        let response = fake_response(100, csv).await;
        assert!(save_report(response, path).await.is_err());
        assert!(!std::path::Path::new(path).exists());
    }
}
