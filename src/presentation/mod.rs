use crate::{
    application::Publisher,
    domain::{
        PostRequest, SyntheticMediaRequest, ThumbnailRequest, UploadsRequest, VideoIdRequest,
        VideoRequest,
    },
    infrastructure::youtube::YouTube,
};
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock},
    tool, tool_handler, tool_router,
};

#[derive(Clone)]
pub struct Server {
    publisher: Publisher<YouTube>,
    tool_router: ToolRouter<Self>,
}
impl Server {
    pub fn new(publisher: Publisher<YouTube>) -> Self {
        Self {
            publisher,
            tool_router: Self::tool_router(),
        }
    }
}
#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {}
fn result(value: anyhow::Result<serde_json::Value>) -> CallToolResult {
    match value {
        Ok(value) => CallToolResult::success(vec![ContentBlock::text(value.to_string())]),
        Err(error) => CallToolResult::error(vec![ContentBlock::text(format!("{error:#}"))]),
    }
}
#[tool_router(router = tool_router)]
impl Server {
    #[tool(description = "Consulta o canal associado à conta OAuth autorizada.")]
    async fn canal(&self) -> CallToolResult {
        result(self.publisher.channel().await)
    }
    #[tool(
        description = "Consulta vídeo por ID, incluindo privacidade, agendamento e processamento quando disponíveis."
    )]
    async fn consultar_video(
        &self,
        Parameters(request): Parameters<VideoIdRequest>,
    ) -> CallToolResult {
        result(self.publisher.get_video(request).await)
    }
    #[tool(
        description = "Lista uma página dos uploads do canal autenticado com detalhes e token para a próxima página; confira todas as páginas ao procurar duplicatas."
    )]
    async fn listar_uploads(
        &self,
        Parameters(request): Parameters<UploadsRequest>,
    ) -> CallToolResult {
        result(self.publisher.list_uploads(request).await)
    }
    #[tool(
        description = "Atualiza a declaração de conteúdo realista alterado ou sintético de um vídeo próprio, preservando o status recuperado. Altera o vídeo no YouTube."
    )]
    async fn declarar_conteudo_sintetico(
        &self,
        Parameters(request): Parameters<SyntheticMediaRequest>,
    ) -> CallToolResult {
        result(self.publisher.set_synthetic_media(request).await)
    }
    #[tool(
        description = "Define ou substitui a miniatura personalizada de um vídeo próprio usando uma imagem JPEG ou PNG local de até 50 MB. Altera o vídeo no YouTube."
    )]
    async fn definir_miniatura(
        &self,
        Parameters(request): Parameters<ThumbnailRequest>,
    ) -> CallToolResult {
        result(self.publisher.set_thumbnail(request).await)
    }
    #[tool(
        description = "Envia um vídeo ou Short ao YouTube; o YouTube decide se é Short. Por padrão é público. Pode agendar com publish_at; vídeos agendados ficam privados até a publicação."
    )]
    async fn publicar_video(
        &self,
        Parameters(request): Parameters<VideoRequest>,
    ) -> CallToolResult {
        result(self.publisher.video(request).await)
    }
    #[tool(
        description = "Publica imediatamente um post público de texto e/ou imagens. A API de posts exige acesso ao piloto do YouTube."
    )]
    async fn publicar_post(&self, Parameters(request): Parameters<PostRequest>) -> CallToolResult {
        result(self.publisher.post(request).await)
    }
}
