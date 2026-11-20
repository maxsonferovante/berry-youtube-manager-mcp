# Licenses and dependencies

Berry's original source and documentation are distributed under the [MIT License](../LICENSE). Third-party components retain their respective licenses. The authoritative resolved dependency versions are in `Cargo.lock`; platform-specific and transitive dependencies may add other licenses.

| Direct dependency | Purpose | Declared license |
| --- | --- | --- |
| `google-cloud-auth` | OAuth user credentials and access tokens | Apache-2.0 |
| `rmcp` | Model Context Protocol SDK | Apache-2.0 |
| `reqwest` | HTTPS client | MIT OR Apache-2.0 |
| `tokio` | Async runtime and IO | MIT |
| `serde`, `serde_json` | Serialization and JSON | MIT OR Apache-2.0 |
| `schemars` | MCP input JSON schemas | MIT |
| `anyhow`, `chrono`, `base64`, `getrandom`, `sha2`, `tokio-util` | Errors, time, encoding, randomness, hashing and IO | Consult the resolved crate metadata. |

Inspect all resolved dependency licenses with `cargo metadata --locked --format-version 1`; do not assume that the table covers every transitive or platform dependency. YouTube and Google are separate services and their documentation and service conditions are not covered by Berry's MIT license.

## Generated diagrams

The standalone HTML diagrams include Archify's embedded viewer and templates. Archify's Apache-2.0 license, historical MIT license, notice and third-party notices are preserved under [`third_party/archify`](../third_party/archify/). The standalone animated SVG exports in [`docs/diagrams`](diagrams/) retain the Archify diagram styling and embedded JetBrains Mono font, distributed under the SIL Open Font License 1.1; its license text is embedded in each SVG and included in each generated HTML. Berry's authored diagram content remains part of this project.
