use crate::domain::{
    PostRequest, SyntheticMediaRequest, ThumbnailRequest, UploadsRequest, VideoIdRequest,
    VideoRequest,
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
}
