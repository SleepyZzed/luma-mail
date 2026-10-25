use crate::{
    Error, Result,
    http::{Http, json},
};
use async_trait::async_trait;
use oauth2::{CsrfToken, PkceCodeChallenge};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
};
use url::Url;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const GMAIL_SCOPE: &str = "https://www.googleapis.com/auth/gmail.modify";
pub const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive.file";

#[derive(Clone, Deserialize)]
pub struct ClientConfig {
    pub client_id: String,
    pub client_secret: String,
}
impl ClientConfig {
    pub fn load(path: &Path) -> Result<Self> {
        #[derive(Deserialize)]
        struct File {
            installed: ClientConfig,
        }
        let metadata = std::fs::metadata(path).map_err(|_| Error::Invalid(
            "Import a Google Desktop OAuth client JSON before signing in. This development preview does not include a public OAuth client.".into()))?;
        if metadata.len() > 64 * 1024 {
            return Err(Error::Invalid("The OAuth client JSON is unexpectedly large. Choose the Desktop app file downloaded from Google Cloud.".into()));
        }
        let data = std::fs::read(path)?;
        let client = serde_json::from_slice::<File>(&data).map_err(|_| Error::Invalid(
            "Choose a valid Google Desktop app OAuth JSON. Web application clients and service-account keys cannot be used for desktop sign-in.".into()))?.installed;
        if client.client_id.trim().is_empty() || client.client_secret.trim().is_empty() {
            return Err(Error::Invalid(
                "The Desktop OAuth client JSON is missing its client ID or client secret.".into(),
            ));
        }
        Ok(client)
    }
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Credentials {
    pub refresh_token: String,
}

#[async_trait]
pub trait Vault: Send + Sync {
    async fn load(&self, account: &str) -> Result<Credentials>;
    async fn store(&self, account: &str, value: &Credentials) -> Result<()>;
    async fn delete(&self, account: &str) -> Result<()>;
}

pub struct SystemVault;

#[cfg(target_os = "linux")]
#[async_trait]
impl Vault for SystemVault {
    async fn load(&self, account: &str) -> Result<Credentials> {
        use secret_service::{EncryptionType, SecretService};
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(keyring_error)?;
        let found = ss
            .search_items(std::collections::HashMap::from([
                ("application", "luma-mail"),
                ("account", account),
            ]))
            .await
            .map_err(keyring_error)?;
        let item = found
            .unlocked
            .first()
            .or_else(|| found.locked.first())
            .ok_or(Error::Reauthenticate)?;
        if item.is_locked().await.map_err(keyring_error)? {
            item.unlock().await.map_err(keyring_error)?;
        }
        let mut bytes = item.get_secret().await.map_err(keyring_error)?;
        let result = serde_json::from_slice(&bytes).map_err(Error::from);
        bytes.zeroize();
        result
    }
    async fn store(&self, account: &str, value: &Credentials) -> Result<()> {
        use secret_service::{EncryptionType, SecretService};
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(keyring_error)?;
        let collection = ss.get_default_collection().await.map_err(keyring_error)?;
        if collection.is_locked().await.map_err(keyring_error)? {
            collection.unlock().await.map_err(keyring_error)?;
        }
        let mut data = serde_json::to_vec(value)?;
        let result = collection
            .create_item(
                "Luma Mail Google authorization",
                std::collections::HashMap::from([
                    ("application", "luma-mail"),
                    ("account", account),
                ]),
                &data,
                true,
                "application/json",
            )
            .await
            .map_err(keyring_error);
        data.zeroize();
        result.map(|_| ())
    }
    async fn delete(&self, account: &str) -> Result<()> {
        use secret_service::{EncryptionType, SecretService};
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(keyring_error)?;
        let found = ss
            .search_items(std::collections::HashMap::from([
                ("application", "luma-mail"),
                ("account", account),
            ]))
            .await
            .map_err(keyring_error)?;
        for item in found.unlocked.iter().chain(found.locked.iter()) {
            item.delete().await.map_err(keyring_error)?;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn keyring_error(_: impl std::fmt::Display) -> Error {
    Error::Keyring(
        "Unlock a Secret Service keyring (GNOME Keyring or KWallet) in your desktop session".into(),
    )
}

#[cfg(not(target_os = "linux"))]
#[async_trait]
impl Vault for SystemVault {
    async fn load(&self, _: &str) -> Result<Credentials> {
        Err(Error::Keyring(
            "Account login requires the Linux Secret Service build".into(),
        ))
    }
    async fn store(&self, _: &str, _: &Credentials) -> Result<()> {
        Err(Error::Keyring(
            "Account login requires the Linux Secret Service build".into(),
        ))
    }
    async fn delete(&self, _: &str) -> Result<()> {
        Err(Error::Keyring(
            "Account login requires the Linux Secret Service build".into(),
        ))
    }
}

#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
    #[serde(default)]
    pub scope: String,
}

pub struct Authorization {
    listener: TcpListener,
    verifier: String,
    state: String,
    redirect: String,
    pub url: Url,
    drive: bool,
}

impl Authorization {
    pub async fn begin(client: &ClientConfig, drive: bool) -> Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let redirect = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let state = CsrfToken::new_random().secret().clone();
        let mut url =
            Url::parse("https://accounts.google.com/o/oauth2/v2/auth").expect("constant URL");
        let scope = if drive {
            format!("{GMAIL_SCOPE} {DRIVE_SCOPE}")
        } else {
            GMAIL_SCOPE.into()
        };
        url.query_pairs_mut().extend_pairs([
            ("client_id", client.client_id.as_str()),
            ("redirect_uri", &redirect),
            ("response_type", "code"),
            ("scope", &scope),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", &state),
            ("access_type", "offline"),
            ("prompt", "consent select_account"),
        ]);
        Ok(Self {
            listener,
            verifier: verifier.secret().clone(),
            state,
            redirect,
            url,
            drive,
        })
    }
    pub async fn complete(mut self, http: &Http, client: &ClientConfig) -> Result<TokenResponse> {
        let result = tokio::time::timeout(Duration::from_secs(300), async {
            loop {
                let (mut socket, _) = self.listener.accept().await?;
                let mut request = vec![0u8; 8192];
                let n = match tokio::time::timeout(Duration::from_secs(3), socket.read(&mut request)).await {
                    Ok(Ok(n)) => n, _ => continue,
                };
                let request = String::from_utf8_lossy(&request[..n]);
                let target = request.lines().next().and_then(|line| line.strip_prefix("GET "))
                    .and_then(|line| line.split_whitespace().next()).unwrap_or("");
                let parsed = Url::parse(&format!("{}{target}", self.redirect));
                let mut code = None;
                let mut denied = false;
                if let Ok(url) = parsed {
                    let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
                    if url.path() == "/" && valid_state(&self.state, params.get("state").map(String::as_str).unwrap_or("")) {
                        code = params.get("code").cloned();
                        denied = params.contains_key("error");
                    }
                }
                let valid = code.is_some() || denied;
                let (status, body) = if valid { ("200 OK", "You may close this tab and return to Luma Mail.") }
                    else { ("400 Bad Request", "This is not a valid Luma Mail sign-in callback.") };
                let reply = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len());
                let _ = socket.write_all(reply.as_bytes()).await;
                if denied { return Err(Error::Invalid("Google sign-in was cancelled or denied".into())); }
                if let Some(code) = code { break Ok(code); }
            }
        }).await.map_err(|_| Error::Timeout)?;
        let code = result?;
        let response = http
            .client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("client_id", client.client_id.as_str()),
                ("client_secret", &client.client_secret),
                ("code", &code),
                ("code_verifier", &self.verifier),
                ("redirect_uri", &self.redirect),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await?;
        self.verifier.zeroize();
        if !response.status().is_success() {
            return Err(Error::Reauthenticate);
        }
        let token: TokenResponse = json(response).await?;
        if !token.scope.split_whitespace().any(|s| s == GMAIL_SCOPE)
            || (self.drive && !token.scope.split_whitespace().any(|s| s == DRIVE_SCOPE))
        {
            return Err(Error::Invalid(
                "Required permissions were not granted by Google".into(),
            ));
        }
        Ok(token)
    }
}

pub fn valid_state(expected: &str, actual: &str) -> bool {
    expected.as_bytes().ct_eq(actual.as_bytes()).into()
}

struct AccessToken {
    token: String,
    expires: Instant,
}
impl Drop for AccessToken {
    fn drop(&mut self) {
        self.token.zeroize();
    }
}

pub struct TokenManager {
    account: String,
    client: ClientConfig,
    http: Http,
    vault: Arc<dyn Vault>,
    access: Mutex<Option<AccessToken>>,
    endpoint: String,
}

impl TokenManager {
    pub fn new(account: String, client: ClientConfig, http: Http, vault: Arc<dyn Vault>) -> Self {
        Self {
            account,
            client,
            http,
            vault,
            access: Mutex::new(None),
            endpoint: "https://oauth2.googleapis.com/token".into(),
        }
    }
    pub async fn access_token(&self) -> Result<String> {
        let mut guard = self.access.lock().await;
        if let Some(access) = guard
            .as_ref()
            .filter(|a| a.expires > Instant::now() + Duration::from_secs(60))
        {
            return Ok(access.token.clone());
        }
        let credential = self.vault.load(&self.account).await?;
        let response = self
            .http
            .client
            .post(&self.endpoint)
            .form(&[
                ("client_id", self.client.client_id.as_str()),
                ("client_secret", &self.client.client_secret),
                ("refresh_token", &credential.refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(Error::Reauthenticate);
        }
        let response: TokenResponse = json(response).await?;
        if let Some(refresh) = response.refresh_token.as_ref() {
            self.vault
                .store(
                    &self.account,
                    &Credentials {
                        refresh_token: refresh.clone(),
                    },
                )
                .await?;
        }
        *guard = Some(AccessToken {
            token: response.access_token.clone(),
            expires: Instant::now() + Duration::from_secs(response.expires_in),
        });
        Ok(response.access_token.clone())
    }
    pub async fn revoke(&self) -> Result<()> {
        let credential = self.vault.load(&self.account).await?;
        let response = self
            .http
            .client
            .post("https://oauth2.googleapis.com/revoke")
            .form(&[("token", &credential.refresh_token)])
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(Error::Invalid("Revocation failed. Retry when online or revoke Luma Mail in your Google Account permissions.".into()));
        }
        self.vault.delete(&self.account).await?;
        *self.access.lock().await = None;
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn test_access(&mut self, token: &str) {
        *self.access.get_mut() = Some(AccessToken {
            token: token.into(),
            expires: Instant::now() + Duration::from_secs(3600),
        });
    }
    #[cfg(test)]
    pub(crate) fn test_endpoint(&mut self, url: String) {
        self.endpoint = url;
    }
}
