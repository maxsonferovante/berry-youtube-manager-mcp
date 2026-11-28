use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

const SCOPES: &str = concat!(
    "https://www.googleapis.com/auth/youtube.force-ssl ",
    "https://www.googleapis.com/auth/youtube.readonly ",
    "https://www.googleapis.com/auth/yt-analytics.readonly ",
    "https://www.googleapis.com/auth/yt-analytics-monetary.readonly"
);
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
pub fn token_path() -> PathBuf {
    std::env::var_os("YOUTUBE_MCP_TOKEN_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| project_dir().join(".private/authorized_user.json"))
}
fn client_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("YOUTUBE_MCP_CLIENT_SECRET") {
        return Ok(path.into());
    }
    let local_client = project_dir().join(".private/client_secret.json");
    if local_client.is_file() {
        return Ok(local_client);
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
pub struct Authorization {
    pub url: url::Url,
    listener: TcpListener,
    redirect: String,
    verifier: String,
    state: String,
    client: Installed,
}

pub struct AuthorizedUser {
    pub access_token: String,
    credential: Value,
}

pub async fn start() -> Result<Authorization> {
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
        .append_pair("scope", SCOPES)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "select_account consent")
        .append_pair("state", &state)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(Authorization {
        url,
        listener,
        redirect,
        verifier,
        state,
        client: secret.installed,
    })
}

async fn callback(stream: &mut TcpStream, state: &str) -> Result<Option<String>> {
    let mut reader = BufReader::new(&mut *stream).take(8192);
    let mut line = String::new();
    if reader.read_line(&mut line).await.is_err() || !line.ends_with('\n') {
        return Ok(None);
    }
    let mut parts = line.split_whitespace();
    if parts.next() != Some("GET") {
        return Ok(None);
    }
    let Some(target) = parts.next() else {
        return Ok(None);
    };
    let Ok(url) = url::Url::parse(&format!("http://127.0.0.1{target}")) else {
        return Ok(None);
    };
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    if url.path() != "/" || query.get("state").is_none_or(|received| received != state) {
        return Ok(None);
    }

    let mut header = Vec::new();
    loop {
        header.clear();
        match reader.read_until(b'\n', &mut header).await {
            Ok(0) | Err(_) => return Ok(None),
            Ok(_) if header == b"\r\n" => break,
            Ok(_) => {}
        }
    }

    let code = query.get("code").filter(|code| !code.is_empty());
    let (status, body) = if code.is_some() && !query.contains_key("error") {
        (
            "200 OK",
            "Resposta recebida. Confira o resultado no cliente MCP ou no terminal.",
        )
    } else {
        (
            "400 Bad Request",
            "O Google não concedeu a autorização. Pode fechar esta janela.",
        )
    };
    let reply = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(reply.as_bytes()).await;
    if query.contains_key("error") {
        bail!("autorização recusada pelo Google");
    }
    Ok(Some(code.context("código OAuth ausente")?.clone()))
}

impl Authorization {
    pub async fn finish(self) -> Result<AuthorizedUser> {
        let code = tokio::time::timeout(Duration::from_secs(300), async {
            loop {
                let (mut stream, _) = self.listener.accept().await?;
                match tokio::time::timeout(
                    Duration::from_secs(2),
                    callback(&mut stream, &self.state),
                )
                .await
                {
                    Ok(Ok(Some(code))) => return Ok::<_, anyhow::Error>(code),
                    Ok(Err(error)) => return Err(error),
                    _ => {}
                }
            }
        })
        .await
        .context("tempo esgotado aguardando a autorização")??;
        let response = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()?
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("code", code.as_str()),
                ("client_id", &self.client.client_id),
                ("client_secret", &self.client.client_secret),
                ("redirect_uri", &self.redirect),
                ("grant_type", "authorization_code"),
                ("code_verifier", &self.verifier),
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
        let access_token = response
            .get("access_token")
            .and_then(Value::as_str)
            .context("Google não retornou access token")?
            .to_owned();
        let credential = json!({"type":"authorized_user", "client_id":self.client.client_id, "client_secret":self.client.client_secret, "refresh_token":refresh});
        Ok(AuthorizedUser {
            access_token,
            credential,
        })
    }
}

impl AuthorizedUser {
    pub async fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("caminho do token inválido")?;
        let data = serde_json::to_vec(&self.credential)?;
        let mut directory = tokio::fs::DirBuilder::new();
        directory.recursive(true);
        #[cfg(unix)]
        directory.mode(0o700);
        directory.create(parent).await?;
        let temporary = parent.join(format!(".oauth-{}.tmp", random_urlsafe()?));
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&temporary).await?;
        let result = async {
            file.write_all(&data).await?;
            file.sync_all().await?;
            drop(file);
            tokio::fs::rename(&temporary, path).await
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result.context("não foi possível salvar a autorização")
    }
}

pub async fn authorize() -> Result<()> {
    let authorization = start().await?;
    eprintln!(
        "Abra este endereço no navegador e autorize o canal:\n{}",
        authorization.url
    );
    let path = token_path();
    authorization.finish().await?.save(&path).await?;
    eprintln!("Autorização salva em {}", path.display());
    Ok(())
}

fn channels_dir() -> PathBuf {
    token_path().with_extension("channels")
}

fn is_channel_id(id: &str) -> bool {
    id.len() == 24
        && id.starts_with("UC")
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub fn channel_path(id: &str) -> Result<PathBuf> {
    if !is_channel_id(id) {
        bail!("informe um ID de canal do YouTube que comece com UC");
    }
    Ok(channels_dir().join(format!("{id}.json")))
}

pub async fn saved_channels() -> Result<Vec<String>> {
    let mut entries = match tokio::fs::read_dir(channels_dir()).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("não foi possível listar canais autorizados"),
    };
    let mut ids = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        if let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            && is_channel_id(id)
            && entry.file_type().await?.is_file()
        {
            ids.push(id.to_owned());
        }
    }
    ids.sort_unstable();
    Ok(ids)
}

pub async fn access_token(path: &Path) -> Result<String> {
    let data = tokio::fs::read(path)
        .await
        .context("credencial indisponível; use iniciar_login e depois selecionar_canal no MCP")?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn callback_ignores_other_requests_and_handles_google_denial() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let client = tokio::spawn(async move {
            for query in [
                "state=other&code=ignored",
                "state=expected&error=access_denied",
            ] {
                let mut stream = TcpStream::connect(address).await?;
                stream
                    .write_all(
                        format!("GET /?{query} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes(),
                    )
                    .await?;
                let mut response = String::new();
                stream.read_to_string(&mut response).await?;
                match query {
                    "state=other&code=ignored" => assert!(response.is_empty()),
                    _ => assert!(response.starts_with("HTTP/1.1 400 Bad Request")),
                }
            }
            Ok::<_, anyhow::Error>(())
        });

        let (mut stream, _) = listener.accept().await?;
        assert_eq!(callback(&mut stream, "expected").await?, None);
        drop(stream);
        let (mut stream, _) = listener.accept().await?;
        let error = callback(&mut stream, "expected").await.unwrap_err();
        assert_eq!(error.to_string(), "autorização recusada pelo Google");
        drop(stream);
        client.await??;
        Ok(())
    }
}
