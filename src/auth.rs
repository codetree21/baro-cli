use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;

use crate::api::BaroClient;
use crate::config;

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
    // Bind to random port
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();

    let auth_url = format!("{}/auth/cli?port={}", config::api_base_url(), port);
    println!("Opening browser for authentication...");
    println!("If the browser doesn't open, visit:\n{}\n", auth_url);

    // Try to open browser
    let _ = open::that(&auth_url);

    println!("Waiting for authentication...");

    // Accept one connection
    let (mut stream, _) = listener.accept()?;
    let reader = BufReader::new(&stream);
    let request_line = reader
        .lines()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No request received"))??;

    // Parse: GET /callback?access_token=...&refresh_token=...&expires_at=... HTTP/1.1
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid HTTP request"))?;

    let query = path
        .split('?')
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("No query parameters in callback"))?;

    let params: std::collections::HashMap<String, String> = query
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            Some((parts.next()?.to_string(), parts.next()?.to_string()))
        })
        .collect();

    let access_token = params
        .get("access_token")
        .ok_or_else(|| anyhow::anyhow!("Missing access_token in callback"))?
        .clone();
    let refresh_token = params
        .get("refresh_token")
        .ok_or_else(|| anyhow::anyhow!("Missing refresh_token in callback"))?
        .clone();
    let expires_at: i64 = params
        .get("expires_at")
        .ok_or_else(|| anyhow::anyhow!("Missing expires_at in callback"))?
        .parse()
        .context("Invalid expires_at value")?;

    // Send success response
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nAccess-Control-Allow-Origin: *\r\n\r\n<html><body><h1>Authentication successful!</h1><p>You can close this window and return to the terminal.</p></body></html>";
    stream.write_all(response.as_bytes())?;
    drop(stream);

    // Save credentials
    let creds = StoredCredentials {
        access_token: access_token.clone(),
        refresh_token,
        expires_at,
    };
    save_credentials(&creds)?;

    // Verify
    let client = BaroClient::new(&access_token);
    let me = client.get_me().await?;
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
