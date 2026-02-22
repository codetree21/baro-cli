use serde::{Deserialize, Serialize};

// -- Auth --

#[derive(Debug, Deserialize)]
pub struct AuthMeResponse {
    pub user: Publisher,
}

#[derive(Debug, Deserialize)]
pub struct Publisher {
    pub id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub personal_tier: String,
    pub storage_used_bytes: i64,
    pub publish_cooldown_until: Option<String>,
}

// -- Categories --

#[derive(Debug, Deserialize)]
pub struct CategoriesResponse {
    pub categories: Vec<Category>,
}

#[derive(Debug, Deserialize)]
pub struct Category {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
}

// -- Products --

#[derive(Debug, Deserialize)]
pub struct ProductsResponse {
    pub products: Vec<Product>,
    pub total: u64,
    pub page: u64,
    pub limit: u64,
}

#[derive(Debug, Deserialize)]
pub struct Product {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub latest_version: Option<String>,
    pub review_status: String,
    pub is_private: bool,
    pub created_at: String,
    pub updated_at: String,
    pub publisher: Option<PublisherRef>,
    pub category: Option<CategoryRef>,
    pub stats: Option<ProductStats>,
}

#[derive(Debug, Deserialize)]
pub struct PublisherRef {
    pub username: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CategoryRef {
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ProductStats {
    #[serde(default)]
    pub fork_count: Option<u64>,
    #[serde(default)]
    pub remake_count: Option<u64>,
    #[serde(default)]
    pub avg_rating: Option<f64>,
    #[serde(default)]
    pub rating_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProductRequest {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub category_slug: String,
    pub license: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProductResponse {
    pub product: Product,
}

// -- Releases --

#[derive(Debug, Deserialize)]
pub struct ReleasesResponse {
    pub releases: Vec<Release>,
}

#[derive(Debug, Deserialize)]
pub struct Release {
    pub id: String,
    pub version: String,
    pub changelog: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateReleaseResponse {
    pub release_id: String,
    pub upload_url: String,
    pub upload_expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmResponse {
    pub release_id: String,
    pub upload_status: String,
    #[serde(default)]
    pub review_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DownloadResponse {
    pub download_url: String,
    pub expires_in: u64,
    pub file_size_bytes: i64,
    pub file_hash_sha256: String,
}

// -- My Products --

#[derive(Debug, Deserialize)]
pub struct MyProductsResponse {
    pub products: Vec<Product>,
}

// -- Error --

#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub error: String,
}

// -- Manifest --

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    // Clone fields (present for cloned products)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloned_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<String>,

    // Publish identity (present for published products)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,

    // Version (always present)
    pub version: String,
}

// -- Supabase token refresh --

#[derive(Debug, Deserialize)]
pub struct SupabaseTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}
