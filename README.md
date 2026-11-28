# Berry YouTube Manager MCP

**Berry is a YouTube channel manager.** A Rust [Model Context Protocol](https://modelcontextprotocol.io/) server that connects an agent to the YouTube Data API through a user-authorized Google account.

[Português](README.pt-BR.md) · [Architecture](docs/architecture.md) · [Operations](docs/operations.md) · [Dependencies and licenses](docs/licenses.md) · [Contributing](CONTRIBUTING.md)

Berry provides channel and video queries, video/Short uploads with captions, statistics and reports, comment management, and share links. It uses local stdio: your MCP client starts the server; there is no HTTP server or per-agent authentication. Local agents able to launch the server can exercise the permissions granted to its Google account.

## Operation diagrams

### OAuth authorization

![Animated Archify diagram of Berry OAuth authorization](docs/diagrams/autorizacao.svg)

[Open the Archify interactive viewer](.archify/workflow-berry-operations-20261120-173118/autorizacao.html)

### Video publication

![Animated Archify diagram of Berry video publication](docs/diagrams/publicacao.svg)

[Open the Archify interactive viewer](.archify/workflow-berry-operations-20261120-173118/publicacao.html)

The SVGs are standalone animated exports of the Archify diagrams. GitHub may display SVG animation as a static image; open the linked HTML viewer locally for the interactive version. Diagram labels are in Portuguese.

## Requirements

- Rust and Cargo with edition 2024 support. The project is tested locally with Rust 1.95; the minimum supported version has not been established.
- A Google Cloud project with YouTube Data API v3, YouTube Analytics API, and YouTube Reporting API enabled and an OAuth **Desktop app** client JSON.
- A Google account with access to the intended channel, a browser, and network access to Google APIs.
- An MCP client supporting local stdio servers, such as Codex or Claude Desktop.

## Getting started

1. Clone your copy of this repository and enter its directory.
2. Enable YouTube Data API v3, YouTube Analytics API, and YouTube Reporting API in Google Cloud. Configure the OAuth consent screen and create a **Desktop app** OAuth client. When the application is in testing mode, add your account as a test user.
3. Build the server with `cargo build --release --locked`. Configure your MCP client with the absolute binary path and set `YOUTUBE_MCP_CLIENT_SECRET` in the server environment to the path of your Desktop OAuth client JSON.
4. In the MCP client, call `iniciar_login`, open the link, and choose the YouTube channel in the Google consent screen. Use the browser on the same machine as Berry so it can reach the temporary listener on `127.0.0.1`.
5. Call `status_login` after consenting. Once it returns `autorizado`, pass the returned channel ID to `selecionar_canal` and use `canal` to verify the selection. To use a saved channel in a new session, run `listar_canais_autorizados` and `selecionar_canal`.

The original `youtube-mcp authorize` command still sets the session's default account. MCP logins are stored by channel ID in the sibling `authorized_user.channels` directory by default. When `YOUTUBE_MCP_TOKEN_FILE` is set, Berry replaces the file extension with `.channels` to locate that directory.

The source checkout path is embedded at build time to locate `.private/authorized_user.json` by default. Set `YOUTUBE_MCP_TOKEN_FILE` to an absolute path if the binary or its build directory may move. The token-file override must be present both during `authorize` and in the MCP client's environment. `.env.example` is a reference only; Berry does not automatically load it.

### Client configuration

Generic JSON configuration (use the format supported by your MCP client):

```json
{
  "mcpServers": {
    "berry-youtube-manager": {
      "command": "/absolute/path/to/berry/target/release/youtube-mcp",
      "args": [],
      "env": {
        "YOUTUBE_MCP_CLIENT_SECRET": "/absolute/path/to/client_secret.json"
      }
    }
  }
}
```

To make Berry available across Codex projects, register it in the global configuration.

```sh
codex mcp add berry-youtube-manager \
  --env "YOUTUBE_MCP_CLIENT_SECRET=/absolute/path/to/client_secret.json" \
  -- "/absolute/path/to/berry/target/release/youtube-mcp"
```

For a single trusted project, put the equivalent configuration in its `.codex/config.toml`.

```toml
[mcp_servers.berry-youtube-manager]
command = "/absolute/path/to/berry/target/release/youtube-mcp"
env = { YOUTUBE_MCP_CLIENT_SECRET = "/absolute/path/to/client_secret.json" }
```

The package is named `berry-youtube-manager-mcp`; the executable remains `youtube-mcp`. MCP protocol messages use stdout. Repeat `cargo build --release --locked` and restart the client after changing the server.

## MCP tools

| Tool | Input | Behavior |
| --- | --- | --- |
| `iniciar_login` | none | Returns the Google consent URL. |
| `status_login` | none | Returns the latest login status and the saved channel identity on success. |
| `listar_canais_autorizados` | none | Lists saved channel IDs and the current session selection. |
| `selecionar_canal` | optional `id` | Selects a saved channel for this session. Omit the ID to use the default account. |
| `canal` | none | Returns the authenticated channel. |
| `consultar_video` | `video_id` | Fetches video details, status and processing when available. |
| `listar_uploads` | optional `max_results` (1–50), `page_token` | Lists one page of uploads and returns `nextPageToken`. Inspect all pages before concluding that a video is absent. |
| `publicar_video` | absolute `file_path`, `title`, `made_for_kids`; optional `description`, `privacy`, `publish_at`, `category_id`, `tags`, `contains_synthetic_media`, `captions` | Uploads a video; the default is public. A scheduled video is private until `publish_at`; use RFC 3339 with an offset. YouTube determines whether a video is a Short. |
| `definir_miniatura` | `video_id`, absolute JPEG or PNG `file_path` | Sets or replaces the custom thumbnail of an owned video; maximum image size is 50 MB. |
| `declarar_conteudo_sintetico` | `video_id`, `contains_synthetic_media` | Updates the altered/synthetic media declaration while retaining known mutable status fields and using an ETag precondition. |
| `anexar_legenda` | `video_id`, `caption` | Uploads a caption track to an existing video, including retry after a failed track upload. |
| `obter_estatisticas` | `resource`, `ids` | Returns raw video, channel or playlist data. |
| `consultar_analytics` | `start_date`, `end_date`, `metrics`, optional filters | Returns raw YouTube Analytics reports with supported metrics, dimensions and pagination. |
| `listar_tipos_relatorios`, `listar_jobs_relatorios` | optional pagination and filters | Lists available bulk-report types and existing jobs. |
| `criar_job_relatorio`, `listar_relatorios`, `baixar_relatorio` | report type/job, filters and CSV path | Creates a job, lists generated reports and downloads raw CSV without overwriting existing files. |
| `listar_comentarios`, `listar_respostas`, `responder_comentario` | IDs, pagination or reply text | Reads video comments and replies and publishes replies. |
| `obter_link_compartilhamento` | `resource`, `id` | Constructs a video, Short or playlist URL without verifying the resource. |

For video upload, an omitted `contains_synthetic_media` is different from `false`. An omitted privacy defaults to `public`, except when `publish_at` is supplied, when it defaults to `private`. See [operation details](docs/operations.md) for examples and limitations.

## Authorization and data

The Desktop OAuth flow uses PKCE and requests `youtube.force-ssl`, `youtube.readonly`, `yt-analytics.readonly`, and `yt-analytics-monetary.readonly` (including access to available financial reports). After upgrading an existing installation, authorize the channel again to grant the additional scopes. Google describes this scope as allowing broad access to channel content; the exposed Berry tools are a smaller set. The refresh credential is saved locally with restrictive permissions on Unix and is read by `google-cloud-auth` to obtain access tokens. Keep the client JSON and token file private; do not commit them.

## Development

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

No real upload is necessary for the unit tests. CI runs these checks on changes. See [contributing](CONTRIBUTING.md) for the project layout.

## License

Original project code and documentation are available under the [MIT License](LICENSE). Dependencies carry their own licenses; see [licenses](docs/licenses.md).
