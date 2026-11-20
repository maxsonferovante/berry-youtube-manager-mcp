# Architecture

Berry is a local stdio MCP server with a small Clean Architecture-inspired structure:

| Module | Responsibility |
| --- | --- |
| `src/main.rs` | Selects the OAuth authorization command or composes the MCP server. |
| `src/domain/` | Request types and validation for videos, uploads, synthetic-media declarations and posts. |
| `src/application/` | Use cases and `YouTubeGateway` port; validates requests before invoking the gateway. |
| `src/presentation/` | MCP tool registration and conversion of results into MCP responses. |
| `src/infrastructure/oauth.rs` | Desktop OAuth with PKCE, loopback callback, local credential storage and token acquisition. |
| `src/infrastructure/youtube.rs` | YouTube Data API HTTP adapter, video uploads, queries, status updates and community posts. |

The host process communicates with the server through standard input and output. There is no per-agent authentication inside Berry. The adapter uses an access token obtained from the authorized local user credential; YouTube receives API requests over HTTPS.

See the interactive operation diagrams linked from [operations](operations.md). They describe the current local source; the generated diagram is not a deployment or monitoring service.
