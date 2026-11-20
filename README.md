# Berry YouTube Manager MCP

**Berry is a YouTube channel manager.** A Rust [Model Context Protocol](https://modelcontextprotocol.io/) server that connects an agent to the YouTube Data API through a user-authorized Google account.

[Português](README.pt-BR.md) · [Architecture](docs/architecture.md) · [Operations](docs/operations.md) · [Dependencies and licenses](docs/licenses.md) · [Contributing](CONTRIBUTING.md)

Berry provides channel and video queries, video/Short uploads, video scheduling, altered-content disclosure, and immediate community posts where the API is available. It uses local stdio: your MCP client starts the server; there is no HTTP server or per-agent authentication. Local agents able to launch the server can exercise the permissions granted to its Google account.

## Operation diagrams

- [OAuth authorization](.archify/workflow-berry-operations-20261120-173118/autorizacao.html)
- [Video publication](.archify/workflow-berry-operations-20261120-173118/publicacao.html)

Open the HTML files locally after cloning. Diagram labels are in Portuguese; viewer controls are in English.

## Requirements

- Rust and Cargo with edition 2024 support. The project is tested locally with Rust 1.95; the minimum supported version has not been established.
- A Google Cloud project with YouTube Data API v3 enabled and an OAuth **Desktop app** client JSON.
- A Google account with access to the intended channel, a browser, and network access to Google APIs.
- An MCP client supporting local stdio servers, such as Codex or Claude Desktop.

## Getting started

1. Clone your copy of this repository and enter its directory.
2. Enable YouTube Data API v3 in Google Cloud. Configure the OAuth consent screen and create a **Desktop app** OAuth client. When the application is in testing mode, add your account as a test user.
3. Download your own client JSON outside the repository, then point Berry to it:

   ```sh
   export YOUTUBE_MCP_CLIENT_SECRET="/absolute/path/to/client_secret.json"
   cargo build --release --locked
   ./target/release/youtube-mcp authorize
   ```

4. Open the URL printed in the terminal, choose the channel, and grant access. The browser returns to a temporary listener on `127.0.0.1`; the terminal reports whether token exchange and storage succeeded.
5. Configure your MCP client using the absolute path to `target/release/youtube-mcp`, then restart the client. Call `canal` to confirm the selected channel before using a write tool.

The source checkout path is embedded at build time to locate `.private/authorized_user.json` by default. Set `YOUTUBE_MCP_TOKEN_FILE` to an absolute path if the binary or its build directory may move. The token-file override must be present both during `authorize` and in the MCP client's environment. `.env.example` is a reference only; Berry does not automatically load it.

### Client configuration

Generic JSON configuration (use the format supported by your MCP client):

```json
{
  "mcpServers": {
    "berry-youtube-manager": {
      "command": "/absolute/path/to/berry/target/release/youtube-mcp",
      "args": []
    }
  }
}
```

For a trusted Codex project, the equivalent project-local `.codex/config.toml` is:

```toml
[mcp_servers.berry-youtube-manager]
command = "/absolute/path/to/berry/target/release/youtube-mcp"
# env = { YOUTUBE_MCP_TOKEN_FILE = "/absolute/path/to/authorized_user.json" }
```

The package is named `berry-youtube-manager-mcp`; the executable remains `youtube-mcp`. MCP protocol messages use stdout. Repeat `cargo build --release --locked` and restart the client after changing the server.

## MCP tools

| Tool | Input | Behavior |
| --- | --- | --- |
| `canal` | none | Returns the authenticated channel. |
| `consultar_video` | `video_id` | Fetches video details, status and processing when available. |
| `listar_uploads` | optional `max_results` (1–50), `page_token` | Lists one page of uploads and returns `nextPageToken`. Inspect all pages before concluding that a video is absent. |
| `publicar_video` | absolute `file_path`, `title`, `made_for_kids`; optional `description`, `privacy`, `publish_at`, `category_id`, `tags`, `contains_synthetic_media` | Uploads a video; the default is public. A scheduled video is private until `publish_at`; use RFC 3339 with an offset. YouTube determines whether a video is a Short. |
| `declarar_conteudo_sintetico` | `video_id`, `contains_synthetic_media` | Updates the altered/synthetic media declaration while retaining known mutable status fields and using an ETag precondition. |
| `publicar_post` | `channel_id`, optional `text`, optional `image_paths` | Publishes a public community post immediately; text or at least one image is required, with at most 10 images. The posts API is a restricted pilot and its availability depends on the Google project. |

For video upload, an omitted `contains_synthetic_media` is different from `false`. An omitted privacy defaults to `public`, except when `publish_at` is supplied, when it defaults to `private`. See [operation details](docs/operations.md) for examples and limitations.

## Authorization and data

The Desktop OAuth flow uses PKCE and requests `youtube.force-ssl`. Google describes this scope as allowing broad access to channel content; the exposed Berry tools are a smaller set. The refresh credential is saved locally with restrictive permissions on Unix and is read by `google-cloud-auth` to obtain access tokens. Keep the client JSON and token file private; do not commit them. Access to community posts also requires the relevant API capability, independently of OAuth consent.

## Development

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

No real upload is necessary for the unit tests. CI runs these checks on changes. See [contributing](CONTRIBUTING.md) for the project layout.

## License

Original project code and documentation are available under the [MIT License](LICENSE). Dependencies carry their own licenses; see [licenses](docs/licenses.md).
