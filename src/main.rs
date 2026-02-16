mod api;
mod auth;
mod cli;
mod config;
mod manifest;
mod packaging;
mod publish_gate;
mod types;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Login => {
            auth::login().await?;
        }
        Commands::Publish {
            version,
            changelog,
            category,
            name,
            description,
            license,
        } => {
            cmd_publish(version, changelog, category, name, description, license).await?;
        }
        Commands::Clone { product } => {
            cmd_clone(&product).await?;
        }
        Commands::Search {
            query,
            category,
            sort,
            limit,
        } => {
            cmd_search(&query, category.as_deref(), &sort, limit).await?;
        }
        Commands::Status => {
            cmd_status()?;
        }
        Commands::Upstream => {
            cmd_upstream().await?;
        }
    }

    Ok(())
}

async fn cmd_publish(
    version: String,
    changelog: Option<String>,
    category: Option<String>,
    name_flag: Option<String>,
    description_flag: Option<String>,
    license: String,
) -> Result<()> {
    let token = auth::get_token().await?;
    let client = api::BaroClient::new(&token);

    // 1. Get publisher info
    let me = client.get_me().await?;
    println!("Publishing as {}...", me.user.username);

    // 2. Extract metadata from build files or flags
    let cwd = std::env::current_dir()?;
    let (detected_name, detected_desc) = detect_metadata(&cwd);
    let product_name = name_flag
        .or(detected_name)
        .unwrap_or_else(|| dir_to_slug(&cwd));
    let product_desc = description_flag
        .or(detected_desc)
        .ok_or_else(|| anyhow::anyhow!(
            "Description required (50+ chars). Use --description or add to your Cargo.toml/package.json."
        ))?;
    let slug = dir_to_slug(&cwd);

    // 3. Resolve category
    let category_slug = match &category {
        Some(c) => c.clone(),
        None => {
            // Check if product already exists (re-publish)
            let my_products = client.list_my_products().await?;
            match my_products.products.iter().find(|p| p.slug == slug) {
                Some(existing) => existing
                    .category
                    .as_ref()
                    .map(|c| c.slug.clone())
                    .unwrap_or_else(|| "developer-tools".to_string()),
                None => {
                    return Err(anyhow::anyhow!(
                        "Category required for first publish. Use --category <slug>.\n\
                        Available: developer-tools, productivity, ai-agents, data-tools, \
                        devops, design-tools, communication, education, finance, other"
                    ));
                }
            }
        }
    };

    // 4. Run publish gate
    let categories = client.list_categories().await?;
    let gate = publish_gate::run(
        &cwd,
        &version,
        &product_desc,
        &category_slug,
        &categories.categories,
    );
    if !gate.passed {
        eprintln!("Publish gate failed:\n");
        for f in &gate.failures {
            eprintln!("  ERROR: {}", f.message);
            eprintln!("  Fix: {}\n", f.ai_fix_prompt);
        }
        std::process::exit(1);
    }
    for w in &gate.warnings {
        eprintln!("  WARN: {}", w.message);
    }

    // 5. Resolve changelog
    let changelog_text = match changelog {
        Some(cl) => cl,
        None => read_changelog(&cwd, &version)
            .unwrap_or_else(|| format!("Release {}", version)),
    };

    // 6. Package
    println!("Packaging...");
    let (archive_bytes, hash) = packaging::create_archive(&cwd)?;
    let size = archive_bytes.len() as i64;
    println!(
        "  Archive: {} ({})",
        format_bytes(size),
        &hash[..12]
    );

    // 7. Create or find product
    let namespace = &me.user.username;
    let my_products = client.list_my_products().await?;
    let existing = my_products.products.iter().find(|p| p.slug == slug);
    if existing.is_none() {
        println!("Creating product {}/{}...", namespace, slug);
        client
            .create_product(&slug, &product_name, &product_desc, &category_slug, &license)
            .await?;
    }

    // 9. Create release
    println!("Uploading v{}...", version);
    let release = client
        .create_release(namespace, &slug, &version, &changelog_text, size, &hash)
        .await?;

    // 10. Upload to R2
    client
        .upload_to_r2(&release.upload_url, &archive_bytes)
        .await?;

    // 11. Confirm
    client.confirm_release(&release.release_id).await?;

    println!(
        "\nPublished {}/{}@{} ({})",
        namespace,
        slug,
        version,
        format_bytes(size)
    );
    println!("Status: pending_review (admin approval required)");

    // Track fork if this is a cloned product
    let cwd_manifest = manifest::read(&cwd);
    if let Ok(m) = cwd_manifest {
        let origin_parts: Vec<&str> = m.origin.splitn(2, '/').collect();
        if origin_parts.len() == 2 {
            // Get the newly created/existing product ID
            let my_products = client.list_my_products().await?;
            if let Some(published) = my_products.products.iter().find(|p| p.slug == slug) {
                match client
                    .track_fork(origin_parts[0], origin_parts[1], &published.id, &m.version)
                    .await
                {
                    Ok(_) => println!("Fork tracked from {}", m.origin),
                    Err(e) => eprintln!("Warning: could not track fork: {}", e),
                }
            }
        }
    }

    Ok(())
}

async fn cmd_clone(product: &str) -> Result<()> {
    // Parse user/slug[@version]
    let (user_slug, version) = if let Some(idx) = product.rfind('@') {
        (&product[..idx], Some(&product[idx + 1..]))
    } else {
        (product, None)
    };

    let parts: Vec<&str> = user_slug.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid product identifier. Use: user/product or user/product@version"
        ));
    }
    let (username, slug) = (parts[0], parts[1]);

    // Require authentication
    let token = match auth::get_token().await {
        Ok(t) => t,
        Err(_) => {
            eprint!("Login required to clone. Open browser to sign up? [Y/n] ");
            std::io::Write::flush(&mut std::io::stderr())?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim().to_lowercase();
            if input.is_empty() || input == "y" || input == "yes" {
                auth::login().await?;
                auth::get_token().await?
            } else {
                return Err(anyhow::anyhow!(
                    "Run 'baro login' to authenticate (instant with GitHub)"
                ));
            }
        }
    };
    let client = api::BaroClient::new(&token);

    // Get product info
    let product_info = client.get_product(username, slug).await?;
    let target_version = match version {
        Some(v) => v.to_string(),
        None => product_info
            .latest_version
            .ok_or_else(|| anyhow::anyhow!("No published releases for {}/{}", username, slug))?,
    };

    // Get download URL
    println!("Downloading {}/{}@{}...", username, slug, target_version);
    let download = client
        .get_download(username, slug, &target_version)
        .await?;

    // Download
    let bytes = client.download_from_r2(&download.download_url).await?;

    // Verify hash
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual_hash = format!("{:x}", hasher.finalize());
    if actual_hash != download.file_hash_sha256 {
        return Err(anyhow::anyhow!(
            "Hash mismatch! Expected: {}, got: {}",
            download.file_hash_sha256,
            actual_hash
        ));
    }

    // Extract
    let dest = std::path::Path::new(slug);
    if dest.exists() {
        return Err(anyhow::anyhow!(
            "Directory '{}' already exists. Remove it first or clone to a different location.",
            slug
        ));
    }
    packaging::extract_archive(&bytes, dest)?;

    // Write manifest
    let m = types::Manifest {
        origin: format!("{}/{}", username, slug),
        version: target_version.clone(),
        cloned_at: chrono::Utc::now().to_rfc3339(),
        file_hash: actual_hash,
    };
    manifest::write(dest, &m)?;

    println!(
        "Cloned {}/{}@{} -> ./{}/  ({})",
        username,
        slug,
        target_version,
        slug,
        format_bytes(bytes.len() as i64)
    );

    Ok(())
}

async fn cmd_search(query: &str, category: Option<&str>, sort: &str, limit: u32) -> Result<()> {
    let client = api::BaroClient::anonymous();
    let resp = client
        .list_products(Some(query), category, sort, limit, 1)
        .await?;

    if resp.products.is_empty() {
        println!("No products found matching '{}'", query);
        return Ok(());
    }

    for p in &resp.products {
        let pub_name = p
            .publisher
            .as_ref()
            .map(|r| r.username.as_str())
            .unwrap_or("?");
        let cat_name = p
            .category
            .as_ref()
            .map(|c| c.slug.as_str())
            .unwrap_or("?");
        let ver = p.latest_version.as_deref().unwrap_or("-");
        let desc = truncate_str(&p.description, 60);

        println!("{}/{:<20} v{:<8} [{}]", pub_name, p.slug, ver, cat_name);
        println!("  {}", desc);

        if let Some(ref stats) = p.stats {
            let dl = stats.download_count.unwrap_or(0);
            let rating = stats
                .avg_rating
                .map(|r| format!("{:.1}/5", r))
                .unwrap_or_else(|| "-".to_string());
            let rc = stats.rating_count.unwrap_or(0);
            println!("  DL: {}  Rating: {} ({})  Updated: {}", dl, rating, rc, &p.updated_at[..10]);
        }
        println!();
    }

    println!("Found {} results (showing {})", resp.total, resp.products.len());
    Ok(())
}

fn cmd_status() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let m = manifest::read(&cwd)?;
    println!("Origin:  {}", m.origin);
    println!("Version: {}", m.version);
    println!("Cloned:  {}", m.cloned_at);
    Ok(())
}

async fn cmd_upstream() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let m = manifest::read(&cwd)?;

    let parts: Vec<&str> = m.origin.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!("Invalid origin in manifest: {}", m.origin));
    }
    let (username, slug) = (parts[0], parts[1]);

    let client = api::BaroClient::anonymous();
    let releases = client.list_releases(username, slug).await?;

    match releases.releases.first() {
        Some(latest) if latest.version != m.version => {
            println!("New version available: {} (current: {})", latest.version, m.version);
            if let Some(ref cl) = latest.changelog {
                let preview = truncate_str(cl, 100);
                println!("  Changelog: {}", preview);
            }
            println!("  Run: baro clone {}@{}", m.origin, latest.version);
        }
        Some(_) => {
            println!("Up to date with upstream ({})", m.version);
        }
        None => {
            println!("No releases found for {}", m.origin);
        }
    }

    Ok(())
}

// -- Helpers --

fn detect_metadata(dir: &std::path::Path) -> (Option<String>, Option<String>) {
    // Try Cargo.toml
    if let Ok(content) = std::fs::read_to_string(dir.join("Cargo.toml")) {
        let name = extract_toml_value(&content, "name");
        let desc = extract_toml_value(&content, "description");
        if name.is_some() || desc.is_some() {
            return (name, desc);
        }
    }
    // Try package.json
    if let Ok(content) = std::fs::read_to_string(dir.join("package.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            let name = v["name"].as_str().map(String::from);
            let desc = v["description"].as_str().map(String::from);
            return (name, desc);
        }
    }
    (None, None)
}

fn extract_toml_value(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("{} ", key)) || trimmed.starts_with(&format!("{}=", key)) {
            if let Some(val) = trimmed.split('=').nth(1) {
                let val = val.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

fn dir_to_slug(dir: &std::path::Path) -> String {
    dir.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
        .replace(' ', "-")
}

fn read_changelog(dir: &std::path::Path, _version: &str) -> Option<String> {
    let path = dir.join("CHANGELOG.md");
    std::fs::read_to_string(path).ok().map(|content| {
        // Take first non-empty section (simplistic)
        content
            .lines()
            .skip_while(|l| l.trim().is_empty() || l.starts_with('#'))
            .take_while(|l| !l.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    })
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > max_chars {
        let truncated: String = chars[..max_chars - 3].iter().collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

fn format_bytes(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
