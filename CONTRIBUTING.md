# Contributing

1. Create an isolated branch in your own clone.
2. Keep domain validation, application ports and infrastructure adapters separated; add tests when they verify meaningful behavior.
3. Run `cargo fmt --all -- --check`, `cargo test --locked`, and `cargo clippy --locked --all-targets -- -D warnings`.
4. Describe behavior changes and test evidence in the pull request. Do not include client secrets, refresh tokens, channel identifiers, private media, or real uploads in tests or examples.

`src/main.rs` composes the application; `domain`, `application`, `infrastructure`, and `presentation` contain its layers. Documentation and operation diagrams live in `docs/` and `.archify/`.
