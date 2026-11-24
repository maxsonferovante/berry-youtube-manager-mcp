use crate::domain::{
    AnalyticsRequest, CaptionRequest, CommentsRequest, DownloadReportRequest, PostRequest,
    RepliesRequest, ReplyRequest, ReportingJobRequest, ReportingListRequest, ReportsListRequest,
    ShareRequest, StatisticsRequest, SyntheticMediaRequest, ThumbnailRequest, UploadsRequest,
    VideoIdRequest, VideoRequest,
};
use anyhow::Result;
use serde_json::Value;

pub trait YouTubeGateway: Clone + Send + Sync + 'static {
    fn channel(&self) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn get_video(
        &self,
        request: VideoIdRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn list_uploads(
        &self,
        request: UploadsRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn set_synthetic_media(
        &self,
        request: SyntheticMediaRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn set_thumbnail(
        &self,
        request: ThumbnailRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn upload_video(
        &self,
        request: VideoRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn publish_post(
        &self,
        request: PostRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn upload_caption(
        &self,
        request: CaptionRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn statistics(
        &self,
        request: StatisticsRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn analytics(
        &self,
        request: AnalyticsRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn comments(
        &self,
        request: CommentsRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn replies(
        &self,
        request: RepliesRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn reply(
        &self,
        request: ReplyRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn report_types(
        &self,
        request: ReportingListRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn reporting_jobs(
        &self,
        request: ReportingListRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn create_reporting_job(
        &self,
        request: ReportingJobRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn reports(
        &self,
        request: ReportsListRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
    fn download_report(
        &self,
        request: DownloadReportRequest,
    ) -> impl std::future::Future<Output = Result<Value>> + Send;
}

#[derive(Clone)]
pub struct Publisher<T: YouTubeGateway> {
    youtube: T,
}
impl<T: YouTubeGateway> Publisher<T> {
    pub fn new(youtube: T) -> Self {
        Self { youtube }
    }
    pub async fn channel(&self) -> Result<Value> {
        self.youtube.channel().await
    }
    pub async fn get_video(&self, request: VideoIdRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.get_video(request).await
    }
    pub async fn list_uploads(&self, request: UploadsRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.list_uploads(request).await
    }
    pub async fn set_synthetic_media(&self, request: SyntheticMediaRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.set_synthetic_media(request).await
    }
    pub async fn set_thumbnail(&self, request: ThumbnailRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.set_thumbnail(request).await
    }
    pub async fn video(&self, request: VideoRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.upload_video(request).await
    }
    pub async fn post(&self, request: PostRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.publish_post(request).await
    }
    pub async fn caption(&self, request: CaptionRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.upload_caption(request).await
    }
    pub async fn statistics(&self, request: StatisticsRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.statistics(request).await
    }
    pub async fn analytics(&self, request: AnalyticsRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.analytics(request).await
    }
    pub async fn comments(&self, request: CommentsRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.comments(request).await
    }
    pub async fn replies(&self, request: RepliesRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.replies(request).await
    }
    pub async fn reply(&self, request: ReplyRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.reply(request).await
    }
    pub fn share(&self, request: ShareRequest) -> Result<Value> {
        request.validate()?;
        Ok(serde_json::json!({"url": request.url(), "id": request.id}))
    }
    pub async fn report_types(&self, request: ReportingListRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.report_types(request).await
    }
    pub async fn reporting_jobs(&self, request: ReportingListRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.reporting_jobs(request).await
    }
    pub async fn create_reporting_job(&self, request: ReportingJobRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.create_reporting_job(request).await
    }
    pub async fn reports(&self, request: ReportsListRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.reports(request).await
    }
    pub async fn download_report(&self, request: DownloadReportRequest) -> Result<Value> {
        request.validate()?;
        self.youtube.download_report(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Clone, Default)]
    struct FakeGateway(Arc<AtomicUsize>);
    impl YouTubeGateway for FakeGateway {
        async fn channel(&self) -> Result<Value> {
            Ok(serde_json::json!({"id":"channel"}))
        }
        async fn get_video(&self, _: VideoIdRequest) -> Result<Value> {
            Ok(serde_json::json!({"id":"video"}))
        }
        async fn list_uploads(&self, _: UploadsRequest) -> Result<Value> {
            Ok(serde_json::json!({"items":[]}))
        }
        async fn set_synthetic_media(&self, _: SyntheticMediaRequest) -> Result<Value> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"id":"video"}))
        }
        async fn set_thumbnail(&self, _: ThumbnailRequest) -> Result<Value> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"id":"thumbnail"}))
        }
        async fn upload_video(&self, _: VideoRequest) -> Result<Value> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"id":"video"}))
        }
        async fn publish_post(&self, _: PostRequest) -> Result<Value> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"id":"post"}))
        }
        async fn upload_caption(&self, _: CaptionRequest) -> Result<Value> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"id":"caption"}))
        }
        async fn statistics(&self, _: StatisticsRequest) -> Result<Value> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"items":[]}))
        }
        async fn analytics(&self, _: AnalyticsRequest) -> Result<Value> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"rows":[]}))
        }
        async fn comments(&self, _: CommentsRequest) -> Result<Value> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"items":[]}))
        }
        async fn replies(&self, _: RepliesRequest) -> Result<Value> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"items":[]}))
        }
        async fn reply(&self, _: ReplyRequest) -> Result<Value> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"id":"reply"}))
        }
        async fn report_types(&self, _: ReportingListRequest) -> Result<Value> {
            Ok(serde_json::json!({}))
        }
        async fn reporting_jobs(&self, _: ReportingListRequest) -> Result<Value> {
            Ok(serde_json::json!({}))
        }
        async fn create_reporting_job(&self, _: ReportingJobRequest) -> Result<Value> {
            Ok(serde_json::json!({}))
        }
        async fn reports(&self, _: ReportsListRequest) -> Result<Value> {
            Ok(serde_json::json!({}))
        }
        async fn download_report(&self, _: DownloadReportRequest) -> Result<Value> {
            Ok(serde_json::json!({}))
        }
    }
    #[tokio::test]
    async fn invalid_video_is_rejected_before_gateway() {
        let fake = FakeGateway::default();
        let publisher = Publisher::new(fake.clone());
        let video = VideoRequest {
            file_path: "relative.mp4".into(),
            title: "Título".into(),
            description: None,
            privacy: None,
            publish_at: None,
            category_id: None,
            tags: None,
            made_for_kids: false,
            contains_synthetic_media: None,
            captions: None,
        };
        assert!(publisher.video(video).await.is_err());
        assert_eq!(fake.0.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn valid_post_reaches_gateway() {
        let fake = FakeGateway::default();
        let publisher = Publisher::new(fake.clone());
        let post = PostRequest {
            channel_id: "UC123".into(),
            text: Some("Olá".into()),
            image_paths: None,
        };
        assert_eq!(
            publisher.post(post).await.expect("valid post")["id"],
            "post"
        );
        assert_eq!(fake.0.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn invalid_reply_does_not_reach_gateway_and_valid_reply_does() {
        let fake = FakeGateway::default();
        let publisher = Publisher::new(fake.clone());
        let invalid = ReplyRequest {
            parent_id: "comment123".into(),
            text: " ".into(),
        };
        assert!(publisher.reply(invalid).await.is_err());
        assert_eq!(fake.0.load(Ordering::SeqCst), 0);
        let valid = ReplyRequest {
            parent_id: "comment123".into(),
            text: "Obrigado!".into(),
        };
        assert_eq!(publisher.reply(valid).await.unwrap()["id"], "reply");
        assert_eq!(fake.0.load(Ordering::SeqCst), 1);
    }
}
