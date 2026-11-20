use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const SCOPE: &str = "https://www.googleapis.com/auth/youtube.force-ssl";
#[derive(Deserialize)]
struct CredentialFile {
    installed: Installed,
}
#[derive(Deserialize)]
struct Installed {
    client_id: String,
    client_secret: String,
}

fn project_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn token_path() -> PathBuf {
    std::env::var_os("YOUTUBE_MCP_TOKEN_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| project_dir().join(".private/authorized_user.json"))
}
fn client_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("YOUTUBE_MCP_CLIENT_SECRET") {
        return Ok(path.into());
    }
    let root = project_dir()
        .parent()
        .context("pasta do projeto não encontrada")?
        .to_path_buf();
    let paths: Vec<_> = std::fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("client_secret_") && n.ends_with(".json"))
        })
        .collect();
    if paths.len() != 1 {
        bail!(
            "defina YOUTUBE_MCP_CLIENT_SECRET com o caminho do JSON OAuth; esperava encontrar exatamente um arquivo client_secret_*.json"
        );
    }
    Ok(paths[0].clone())
}
fn random_urlsafe() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("não foi possível gerar valor aleatório seguro"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
pub async fn authorize() -> Result<()> {
    let secret: CredentialFile = serde_json::from_slice(&tokio::fs::read(client_path()?).await?)
        .context("JSON deve ser de um cliente OAuth do tipo aplicativo instalado")?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let redirect = format!("http://127.0.0.1:{}/", listener.local_addr()?.port());
    let verifier = random_urlsafe()?;
    let state = random_urlsafe()?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url = url::Url::parse("https://accounts.google.com/o/oauth2/v2/auth")?;
    url.query_pairs_mut()
        .append_pair("client_id", &secret.installed.client_id)
        .append_pair("redirect_uri", &redirect)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("state", &state)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    eprintln!("Abra este endereço no navegador e autorize o canal:\n{url}");
    let (mut stream, _) =
        tokio::time::timeout(std::time::Duration::from_secs(300), listener.accept())
            .await
            .context("tempo esgotado aguardando a autorização")??;
    let mut buf = [0_u8; 8192];
    let n = stream.read(&mut buf).await?;
    let line = std::str::from_utf8(&buf[..n])?
        .lines()
        .next()
        .context("resposta OAuth vazia")?;
    let mut parts = line.split_whitespace();
    if parts.next() != Some("GET") {
        bail!("retorno OAuth inválido");
    }
    let target = parts.next().context("retorno OAuth inválido")?;
    let callback = url::Url::parse(&format!("http://127.0.0.1{target}"))?;
    let query: std::collections::HashMap<_, _> = callback.query_pairs().into_owned().collect();
    if callback.path() != "/" || query.get("state") != Some(&state) {
        bail!("estado OAuth inválido");
    }
    if query.contains_key("error") {
        bail!("autorização recusada pelo Google");
    }
    let code = query.get("code").context("código OAuth ausente")?;
    let body = "Autorização recebida. Pode fechar esta janela.";
    stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await?;
    let response = reqwest::Client::new()
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code.as_str()),
            ("client_id", &secret.installed.client_id),
            ("client_secret", &secret.installed.client_secret),
            ("redirect_uri", &redirect),
            ("grant_type", "authorization_code"),
            ("code_verifier", &verifier),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    let refresh = response
        .get("refresh_token")
        .and_then(Value::as_str)
        .context("Google não retornou refresh token; tente autorizar novamente")?;
    let credential = json!({"type":"authorized_user", "client_id":secret.installed.client_id, "client_secret":secret.installed.client_secret, "refresh_token":refresh});
    let path = token_path();
    let parent = path.parent().context("caminho do token inválido")?;
    std::fs::create_dir_all(parent)?;
    #[cfg(unix)]
    if parent == project_dir().join(".private") {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    use std::io::Write;
    file.write_all(&serde_json::to_vec(&credential)?)?;
    eprintln!("Autorização salva em {}", path.display());
    Ok(())
}
pub async fn access_token() -> Result<String> {
    let path = token_path();
    let data = tokio::fs::read(&path).await.with_context(|| {
        format!(
            "credencial autorizada ausente em {}; execute youtube-mcp authorize",
            path.display()
        )
    })?;
    let value: Value = serde_json::from_slice(&data)?;
    let credentials = google_cloud_auth::credentials::user_account::Builder::new(value)
        .build_access_token_credentials()
        .context("credencial Google inválida")?;
    let token = credentials
        .access_token()
        .await
        .context("não foi possível renovar a autorização Google")?;
    Ok(token.token.to_string())
}
