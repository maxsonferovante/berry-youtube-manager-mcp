# Berry YouTube Manager MCP

**Berry é um gerenciador de canais do YouTube.** Um servidor [MCP](https://modelcontextprotocol.io/) em Rust que conecta agentes à YouTube Data API usando uma conta Google autorizada pelo usuário.

[English](README.md) · [Arquitetura](docs/architecture.md) · [Operações e ferramentas](docs/operations.md) · [Dependências e licenças](docs/licenses.md) · [Contribuição](CONTRIBUTING.md)

O Berry consulta canais e vídeos, envia vídeos e Shorts, agenda vídeos, declara conteúdo sintético e publica posts da comunidade quando a API estiver disponível. O cliente MCP inicia o processo local por stdio. Não há servidor HTTP nem autenticação separada por agente: agentes locais com acesso ao processo podem usar as permissões da conta autorizada.

## Diagramas de operação

### Autorização OAuth

![Diagrama animado do Archify para autorização OAuth do Berry](docs/diagrams/autorizacao.svg)

[Abrir o visualizador interativo do Archify](.archify/workflow-berry-operations-20261120-173118/autorizacao.html)

### Publicação de vídeo

![Diagrama animado do Archify para publicação de vídeo](docs/diagrams/publicacao.svg)

[Abrir o visualizador interativo do Archify](.archify/workflow-berry-operations-20261120-173118/publicacao.html)

Os SVGs são exportações animadas e independentes dos diagramas feitos no Archify. O GitHub pode exibi-los como imagens estáticas; abra o visualizador HTML indicado para ver a versão interativa. Os diagramas estão em português.

## Requisitos

- Rust e Cargo com suporte à edição 2024. O projeto foi testado com Rust 1.95; a versão mínima ainda não foi estabelecida.
- Projeto Google Cloud com YouTube Data API v3 habilitada e um cliente OAuth do tipo **aplicativo para computador**.
- Conta Google com acesso ao canal, navegador e rede para as APIs do Google.
- Cliente MCP com suporte a servidores locais stdio.

## Instalação e autorização

1. Clone sua cópia do repositório e entre na pasta.
2. Habilite a YouTube Data API v3 no Google Cloud. Configure a tela de consentimento e crie um cliente OAuth de aplicativo para computador. Se o aplicativo estiver em teste, adicione sua conta como testadora.
3. Baixe o JSON do seu cliente fora do repositório e informe o caminho:

   ```sh
   export YOUTUBE_MCP_CLIENT_SECRET="/caminho/absoluto/client_secret.json"
   cargo build --release --locked
   ./target/release/youtube-mcp authorize
   ```

4. Abra a URL apresentada, escolha o canal e conceda acesso. O navegador retorna a um receptor temporário em `127.0.0.1`; confira no terminal se a troca e o armazenamento do token terminaram.
5. Configure o cliente MCP com o caminho absoluto de `target/release/youtube-mcp`, reinicie-o e consulte `canal` para conferir a conta antes de publicar.

Por padrão, o caminho da pasta de código no momento da compilação é usado para localizar `.private/authorized_user.json`. Se mover o executável ou a pasta de compilação, defina `YOUTUBE_MCP_TOKEN_FILE` com um caminho absoluto tanto na autorização quanto no ambiente do cliente MCP. O arquivo `.env.example` é apenas referência; o servidor não o carrega automaticamente.

## Conexão ao cliente

Exemplo genérico em JSON:

```json
{
  "mcpServers": {
    "berry-youtube-manager": {
      "command": "/caminho/absoluto/berry/target/release/youtube-mcp",
      "args": []
    }
  }
}
```

Para um projeto confiável no Codex, use `.codex/config.toml`:

```toml
[mcp_servers.berry-youtube-manager]
command = "/caminho/absoluto/berry/target/release/youtube-mcp"
# env = { YOUTUBE_MCP_TOKEN_FILE = "/caminho/absoluto/authorized_user.json" }
```

O pacote Cargo chama-se `berry-youtube-manager-mcp`; o executável permanece `youtube-mcp`. Recompile com `cargo build --release --locked` e reinicie o cliente depois de alterar o servidor.

## Ferramentas MCP

| Ferramenta | Entrada | Comportamento |
| --- | --- | --- |
| `canal` | nenhuma | Consulta o canal autorizado. |
| `consultar_video` | `video_id` | Consulta detalhes, status e processamento quando disponíveis. |
| `listar_uploads` | `max_results` (1–50) e `page_token`, opcionais | Lista uma página de uploads e retorna `nextPageToken`. Consulte todas as páginas antes de concluir que um vídeo não existe. |
| `publicar_video` | `file_path` absoluto, `title`, `made_for_kids`; opcionais `description`, `privacy`, `publish_at`, `category_id`, `tags`, `contains_synthetic_media` | Envia vídeo ou Short; o padrão é público. Para agendamento, usa privacidade privada até a data RFC 3339 informada. O YouTube decide se é Short. |
| `declarar_conteudo_sintetico` | `video_id`, `contains_synthetic_media` | Atualiza a declaração preservando os campos conhecidos do status e usando ETag. |
| `publicar_post` | `channel_id`, `text` e `image_paths` opcionais | Publica imediatamente um post público. Exige texto ou imagem, no máximo dez imagens e acesso ao piloto da API. |

Omitir `contains_synthetic_media` é diferente de informar `false`. Veja exemplos e limitações em [operações](docs/operations.md).

## Autorização e dados

O fluxo OAuth para computador usa PKCE e solicita `youtube.force-ssl`. O Google descreve esse escopo como amplo; as ferramentas expostas pelo Berry são um subconjunto das ações permitidas. O token de renovação fica local e tem permissões restritas no Unix. Não compartilhe nem envie ao GitHub os arquivos de credenciais. O acesso aos posts depende também da disponibilidade da API para o projeto Google.

## Desenvolvimento

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Os testes não exigem upload real. Consulte [contribuição](CONTRIBUTING.md).

## Licença

O código e a documentação originais usam a [licença MIT](LICENSE). Dependências têm licenças próprias: veja [licenças](docs/licenses.md).
