use anyhow::Result;
use std::path::PathBuf;

const DEFAULT_API_BASE: &str = "https://baro-sync.com";
const DEFAULT_SUPABASE_URL: &str = "https://pgelndcxijcplmsyvqwo.supabase.co";
const DEFAULT_SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InBnZWxuZGN4aWpjcGxtc3l2cXdvIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzEwMjQzNDksImV4cCI6MjA4NjYwMDM0OX0.vfeb2hY9TZ0Nuu29ixOrEI95kiLkmXZJAp019CbFWFs";

pub fn config_dir() -> Result<PathBuf> {
    let dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
        .join(".config")
        .join("baro");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn credentials_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("credentials.json"))
}

pub fn api_base_url() -> String {
    std::env::var("BARO_API_URL").unwrap_or_else(|_| DEFAULT_API_BASE.to_string())
}

pub fn supabase_url() -> String {
    std::env::var("BARO_SUPABASE_URL").unwrap_or_else(|_| DEFAULT_SUPABASE_URL.to_string())
}

pub fn supabase_anon_key() -> String {
    std::env::var("BARO_SUPABASE_ANON_KEY")
        .unwrap_or_else(|_| DEFAULT_SUPABASE_ANON_KEY.to_string())
}
