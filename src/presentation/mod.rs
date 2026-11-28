use crate::{
    application::Publisher,
    domain::{
        AnalyticsRequest, CaptionRequest, CommentsRequest, DownloadReportRequest, RepliesRequest,
        ReplyRequest, ReportingJobRequest, ReportingListRequest, ReportsListRequest, ShareRequest,
        StatisticsRequest, SyntheticMediaRequest, ThumbnailRequest, UploadsRequest, VideoIdRequest,
        VideoRequest,
    },
    infrastructure::{oauth, youtube::YouTube},
};
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock},
    tool, tool_handler, tool_router,
};

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::{sync::Mutex, task::JoinHandle};

enum Login {
    Pending {
        url: String,
        task: JoinHandle<anyhow::Result<Value>>,
    },
    Finished {
        status: Value,
    },
}

#[derive(Deserialize, JsonSchema)]
struct ChannelSelection {
    /// ID do canal salvo. Omita para voltar à autorização padrão do terminal.
    id: Option<String>,
}

#[derive(Clone)]
pub struct Server {
    publisher: Publisher<YouTube>,
    youtube: YouTube,
    login: Arc<Mutex<Option<Login>>>,
    tool_router: ToolRouter<Self>,
}
impl Server {
    pub fn new(youtube: YouTube) -> Self {
        Self {
            publisher: Publisher::new(youtube.clone()),
            youtube,
            login: Arc::default(),
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
    #[tool(
        description = "Fornece um link de consentimento do Google válido por cinco minutos. Mostre o link ao usuário para que ele escolha o canal no navegador e depois use status_login."
    )]
    async fn iniciar_login(&self) -> CallToolResult {
        result(self.start_login().await)
    }

    async fn start_login(&self) -> anyhow::Result<Value> {
        let mut login = self.login.lock().await;
        if let Some(Login::Pending { url, task }) = login.as_ref()
            && !task.is_finished()
        {
            return Ok(json!({"status": "aguardando", "url": url}));
        }
        let authorization = oauth::start().await?;
        let url = authorization.url.to_string();
        let youtube = self.youtube.clone();
        let task = tokio::spawn(async move { youtube.save_login(authorization).await });
        *login = Some(Login::Pending {
            url: url.clone(),
            task,
        });
        Ok(json!({"status": "aguardando", "url": url, "expira_em_segundos": 300}))
    }

    #[tool(
        description = "Consulta o resultado de iniciar_login. Quando autorizado, retorna o ID e o título do canal salvo. Use selecionar_canal para passar a operar nesse canal."
    )]
    async fn status_login(&self) -> CallToolResult {
        let mut login = self.login.lock().await;
        if let Some(Login::Pending { task, .. }) = login.as_mut()
            && task.is_finished()
        {
            let status = match task.await {
                Ok(Ok(channel)) => json!({"status": "autorizado", "canal": channel}),
                Ok(Err(error)) => json!({"status": "falhou", "erro": error.to_string()}),
                Err(_) => {
                    json!({"status": "falhou", "erro": "a tarefa de autorização foi interrompida"})
                }
            };
            *login = Some(Login::Finished { status });
        }

        result(Ok(match login.as_ref() {
            None => json!({"status": "nao_iniciado"}),
            Some(Login::Pending { .. }) => json!({"status": "aguardando"}),
            Some(Login::Finished { status }) => status.clone(),
        }))
    }

    #[tool(
        description = "Lista os IDs dos canais salvos pelo login MCP e indica o canal selecionado nesta sessão. Canal selecionado nulo indica uso da autorização padrão, se configurada."
    )]
    async fn listar_canais_autorizados(&self) -> CallToolResult {
        let active = self.youtube.active_channel().await;
        result(
            oauth::saved_channels()
                .await
                .map(|ids| json!({"canais": ids, "canal_selecionado": active})),
        )
    }

    #[tool(
        description = "Seleciona um canal salvo pelo ID para as próximas operações nesta sessão MCP. Omita o ID para voltar à autorização padrão. Retorna o ID e o título confirmados pelo YouTube."
    )]
    async fn selecionar_canal(
        &self,
        Parameters(selection): Parameters<ChannelSelection>,
    ) -> CallToolResult {
        result(self.youtube.select_channel(selection.id).await)
    }

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
        description = "Envia um vídeo ou Short e faixas de legenda opcionais ao YouTube; o YouTube decide se é Short. Se uma legenda falhar, o resultado contém o ID do vídeo já enviado para evitar uploads duplicados."
    )]
    async fn publicar_video(
        &self,
        Parameters(request): Parameters<VideoRequest>,
    ) -> CallToolResult {
        result(self.publisher.video(request).await)
    }
    #[tool(
        description = "Envia uma faixa de legenda com marcação de tempo para um vídeo existente; útil para repetir uma legenda que falhou após o upload."
    )]
    async fn anexar_legenda(
        &self,
        Parameters(request): Parameters<CaptionRequest>,
    ) -> CallToolResult {
        result(self.publisher.caption(request).await)
    }
    #[tool(
        description = "Retorna a resposta bruta da YouTube Data API para estatísticas de vídeos ou canais, ou detalhes de playlists (incluindo quantidade de itens)."
    )]
    async fn obter_estatisticas(
        &self,
        Parameters(request): Parameters<StatisticsRequest>,
    ) -> CallToolResult {
        result(self.publisher.statistics(request).await)
    }
    #[tool(
        description = "Consulta qualquer combinação suportada de métricas, dimensões e filtros da YouTube Analytics API e retorna o JSON bruto. Exige dados do canal autorizado e escopos Analytics."
    )]
    async fn consultar_analytics(
        &self,
        Parameters(request): Parameters<AnalyticsRequest>,
    ) -> CallToolResult {
        result(self.publisher.analytics(request).await)
    }
    #[tool(
        description = "Lista uma página de comentários de um vídeo; respostas embutidas podem ser parciais. Retorna o JSON bruto e nextPageToken."
    )]
    async fn listar_comentarios(
        &self,
        Parameters(request): Parameters<CommentsRequest>,
    ) -> CallToolResult {
        result(self.publisher.comments(request).await)
    }
    #[tool(
        description = "Lista uma página das respostas a um comentário principal e retorna o JSON bruto e nextPageToken."
    )]
    async fn listar_respostas(
        &self,
        Parameters(request): Parameters<RepliesRequest>,
    ) -> CallToolResult {
        result(self.publisher.replies(request).await)
    }
    #[tool(description = "Publica uma resposta a um comentário principal existente no YouTube.")]
    async fn responder_comentario(
        &self,
        Parameters(request): Parameters<ReplyRequest>,
    ) -> CallToolResult {
        result(self.publisher.reply(request).await)
    }
    #[tool(
        description = "Gera um link de compartilhamento de vídeo, Short ou playlist a partir do ID, sem verificar se o recurso existe."
    )]
    async fn obter_link_compartilhamento(
        &self,
        Parameters(request): Parameters<ShareRequest>,
    ) -> CallToolResult {
        result(self.publisher.share(request))
    }
    #[tool(
        description = "Lista os tipos disponíveis de relatórios em massa da YouTube Reporting API; retorna JSON bruto e paginação."
    )]
    async fn listar_tipos_relatorios(
        &self,
        Parameters(request): Parameters<ReportingListRequest>,
    ) -> CallToolResult {
        result(self.publisher.report_types(request).await)
    }
    #[tool(
        description = "Lista jobs de relatórios em massa existentes; retorna JSON bruto e paginação."
    )]
    async fn listar_jobs_relatorios(
        &self,
        Parameters(request): Parameters<ReportingListRequest>,
    ) -> CallToolResult {
        result(self.publisher.reporting_jobs(request).await)
    }
    #[tool(
        description = "Cria um job que solicita ao YouTube a geração diária de um tipo de relatório em massa. O primeiro relatório não fica pronto imediatamente."
    )]
    async fn criar_job_relatorio(
        &self,
        Parameters(request): Parameters<ReportingJobRequest>,
    ) -> CallToolResult {
        result(self.publisher.create_reporting_job(request).await)
    }
    #[tool(
        description = "Lista relatórios em massa já gerados para um job, com filtros e paginação; retorna JSON bruto."
    )]
    async fn listar_relatorios(
        &self,
        Parameters(request): Parameters<ReportsListRequest>,
    ) -> CallToolResult {
        result(self.publisher.reports(request).await)
    }
    #[tool(
        description = "Baixa um relatório em massa em CSV bruto para um novo arquivo local, sem sobrescrever arquivos existentes."
    )]
    async fn baixar_relatorio(
        &self,
        Parameters(request): Parameters<DownloadReportRequest>,
    ) -> CallToolResult {
        result(self.publisher.download_report(request).await)
    }
}
