use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;

use crate::api::BaroClient;
use crate::config;

const LOGIN_TIMEOUT_SECS: u64 = 120;
const POLL_INTERVAL_SECS: u64 = 2;

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

fn save_credentials(creds: &StoredCredentials) -> Result<()> {
    let path = config::credentials_path()?;
    std::fs::write(&path, serde_json::to_string_pretty(creds)?)?;
    let mut perms = std::fs::metadata(&path)?.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(&path, perms)?;
    Ok(())
}

fn load_credentials() -> Result<StoredCredentials> {
    let path = config::credentials_path()?;
    let content = std::fs::read_to_string(&path)
        .context("Not authenticated. Run 'baro login' first.")?;
    let creds: StoredCredentials = serde_json::from_str(&content)?;
    Ok(creds)
}

pub async fn login() -> Result<()> {
    let session_code = uuid::Uuid::new_v4().to_string();
    let base = config::api_base_url();

    // Register session on server
    let client = reqwest::Client::new();
    let resp = client
        .put(format!("{}/api/auth/cli-session", base))
        .json(&serde_json::json!({ "session_code": session_code }))
        .send()
        .await
        .context("Failed to connect to server")?;
    if !resp.status().is_success() {
        anyhow::bail!("Failed to create login session. Try again later.");
    }

    let auth_url = format!("{}/auth/cli?code={}", base, session_code);
    println!("Opening browser for authentication...");
    println!("If the browser doesn't open, visit:\n{}\n", auth_url);

    let _ = open::that(&auth_url);

    println!("Waiting for authentication...");

    // Poll for tokens
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_secs(LOGIN_TIMEOUT_SECS);

    let creds: StoredCredentials = loop {
        if std::time::Instant::now() > deadline {
            anyhow::bail!(
                "Login timed out after {}s. Run 'baro login' to try again.",
                LOGIN_TIMEOUT_SECS
            );
        }

        tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;

        let resp = client
            .get(format!("{}/api/auth/cli-session", base))
            .query(&[("code", &session_code)])
            .send()
            .await
            .context("Failed to connect to server")?;

        match resp.status().as_u16() {
            200 => break resp.json().await.context("Failed to parse auth response")?,
            202 => continue, // pending
            404 | 410 => anyhow::bail!(
                "Login session expired. Run 'baro login' to try again."
            ),
            status => anyhow::bail!("Unexpected server response ({}). Try again.", status),
        }
    };

    save_credentials(&creds)?;

    // Verify
    let api = BaroClient::new(&creds.access_token);
    let me = api.get_me().await
        .context("Tokens received but verification failed.\nRun 'baro login' to try again.")?;
    println!("Authenticated as {}", me.user.username);

    Ok(())
}

pub async fn get_token() -> Result<String> {
    let creds = load_credentials()?;

    let now = chrono::Utc::now().timestamp();

    // Auto-refresh if expiring within 5 minutes
    if now >= creds.expires_at - 300 {
        return refresh_token(&creds).await;
    }

    Ok(creds.access_token)
}

async fn refresh_token(creds: &StoredCredentials) -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "{}/auth/v1/token?grant_type=refresh_token",
            config::supabase_url()
        ))
        .header("apikey", config::supabase_anon_key())
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "refresh_token": creds.refresh_token,
        }))
        .send()
        .await?
        .error_for_status()
        .context("Token refresh failed. Run 'baro login' to re-authenticate.")?;

    let body: serde_json::Value = resp.json().await?;
    let new_creds = StoredCredentials {
        access_token: body["access_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No access_token in refresh response"))?
            .to_string(),
        refresh_token: body["refresh_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No refresh_token in refresh response"))?
            .to_string(),
        expires_at: body["expires_at"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("No expires_at in refresh response"))?,
    };

    let token = new_creds.access_token.clone();
    save_credentials(&new_creds)?;

    Ok(token)
}
