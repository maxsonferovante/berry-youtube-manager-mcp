mod application;
mod domain;
mod infrastructure;
mod presentation;

use anyhow::Result;
use infrastructure::{oauth, youtube::YouTube};
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("authorize") {
        return oauth::authorize().await;
    }
    presentation::Server::new(YouTube::new()?)
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
