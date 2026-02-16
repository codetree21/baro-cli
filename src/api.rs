use anyhow::{Context, Result};

use crate::config;
use crate::types::*;

pub struct BaroClient {
    client: reqwest::Client,
    token: Option<String>,
}

impl BaroClient {
    pub fn new(token: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            token: Some(token.to_string()),
        }
    }

    pub fn anonymous() -> Self {
        Self {
            client: reqwest::Client::new(),
            token: None,
        }
    }

    fn base_url(&self) -> String {
        config::api_base_url()
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url(), path);
        let mut req = self.client.get(&url);
        if let Some(ref token) = self.token {
            req = req.bearer_auth(token);
        }
        let resp = req.send().await.context(format!("Failed to connect: GET {}", path))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body: ApiError = resp.json().await.unwrap_or(ApiError {
                error: format!("HTTP {}", status),
            });
            return Err(anyhow::anyhow!("{}", body.error));
        }
        let data = resp.json().await.context("Failed to parse response")?;
        Ok(data)
    }

    async fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url(), path);
        let mut req = self.client.post(&url).json(body);
        if let Some(ref token) = self.token {
            req = req.bearer_auth(token);
        }
        let resp = req.send().await.context(format!("Failed to connect: POST {}", path))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body: ApiError = resp.json().await.unwrap_or(ApiError {
                error: format!("HTTP {}", status),
            });
            return Err(anyhow::anyhow!("{}", body.error));
        }
        let data = resp.json().await.context("Failed to parse response")?;
        Ok(data)
    }

    // -- Auth --

    pub async fn get_me(&self) -> Result<AuthMeResponse> {
        self.get_json("/api/auth/me").await
    }

    // -- Products --

    pub async fn list_products(
        &self,
        q: Option<&str>,
        category: Option<&str>,
        sort: &str,
        limit: u32,
        page: u32,
    ) -> Result<ProductsResponse> {
        let mut params = vec![
            format!("sort={}", sort),
            format!("limit={}", limit),
            format!("page={}", page),
        ];
        if let Some(q) = q {
            params.push(format!(
                "q={}",
                urlencoded(q)
            ));
        }
        if let Some(cat) = category {
            params.push(format!("category={}", cat));
        }
        let path = format!("/api/products?{}", params.join("&"));
        self.get_json(&path).await
    }

    pub async fn get_product(&self, username: &str, slug: &str) -> Result<Product> {
        #[derive(serde::Deserialize)]
        struct Resp {
            product: Product,
        }
        let resp: Resp = self
            .get_json(&format!("/api/products/{}/{}", username, slug))
            .await?;
        Ok(resp.product)
    }

    pub async fn list_my_products(&self) -> Result<MyProductsResponse> {
        self.get_json("/api/products/me").await
    }

    pub async fn create_product(
        &self,
        slug: &str,
        name: &str,
        description: &str,
        category_slug: &str,
        license: &str,
    ) -> Result<CreateProductResponse> {
        let body = serde_json::json!({
            "slug": slug,
            "name": name,
            "description": description,
            "category_slug": category_slug,
            "license": license,
        });
        self.post_json("/api/products", &body).await
    }

    // -- Releases --

    pub async fn list_releases(&self, username: &str, slug: &str) -> Result<ReleasesResponse> {
        self.get_json(&format!(
            "/api/products/{}/{}/releases",
            username, slug
        ))
        .await
    }

    pub async fn create_release(
        &self,
        username: &str,
        slug: &str,
        version: &str,
        changelog: &str,
        file_size_bytes: i64,
        file_hash_sha256: &str,
    ) -> Result<CreateReleaseResponse> {
        self.post_json(
            &format!("/api/products/{}/{}/releases", username, slug),
            &serde_json::json!({
                "version": version,
                "changelog": changelog,
                "file_size_bytes": file_size_bytes,
                "file_hash_sha256": file_hash_sha256,
            }),
        )
        .await
    }

    pub async fn confirm_release(&self, release_id: &str) -> Result<ConfirmResponse> {
        self.post_json(
            &format!("/api/releases/{}/confirm", release_id),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn get_download(
        &self,
        username: &str,
        slug: &str,
        version: &str,
    ) -> Result<DownloadResponse> {
        self.get_json(&format!(
            "/api/products/{}/{}/releases/{}/download",
            username, slug, version
        ))
        .await
    }

    // -- Forks --

    pub async fn track_fork(
        &self,
        origin_username: &str,
        origin_slug: &str,
        product_id: &str,
        origin_version: &str,
    ) -> Result<serde_json::Value> {
        self.post_json(
            &format!("/api/products/{}/{}/fork", origin_username, origin_slug),
            &serde_json::json!({
                "product_id": product_id,
                "origin_version": origin_version,
            }),
        )
        .await
    }

    // -- Categories --

    pub async fn list_categories(&self) -> Result<CategoriesResponse> {
        self.get_json("/api/categories").await
    }

    // -- R2 direct operations --

    pub async fn upload_to_r2(&self, upload_url: &str, data: &[u8]) -> Result<()> {
        let resp = self
            .client
            .put(upload_url)
            .header("Content-Type", "application/gzip")
            .body(data.to_vec())
            .send()
            .await
            .context("Failed to upload to storage")?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "Upload failed with status {}",
                resp.status()
            ));
        }
        Ok(())
    }

    pub async fn download_from_r2(&self, download_url: &str) -> Result<Vec<u8>> {
        let resp = self
            .client
            .get(download_url)
            .send()
            .await
            .context("Failed to download from storage")?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "Download failed with status {}",
                resp.status()
            ));
        }

        let bytes = resp.bytes().await?.to_vec();
        Ok(bytes)
    }
}

fn urlencoded(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}
