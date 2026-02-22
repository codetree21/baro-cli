mod api;
mod auth;
mod cli;
mod config;
mod manifest;
mod packaging;
mod publish_gate;
mod types;
mod update_check;
mod utils;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let update_handle = update_check::spawn_check();

    let result = match cli.command {
        Commands::Login => {
            auth::login().await
        }
        Commands::Publish {
            version,
            changelog,
            category,
            name,
            description,
            license,
        } => {
            cmd_publish(version, changelog, category, name, description, license).await
        }
        Commands::Remake {
            version,
            slug,
            changelog,
            category,
            name,
            description,
            license,
        } => {
            cmd_remake(version, slug, changelog, category, name, description, license).await
        }
        Commands::Fork { product } | Commands::Clone { product } => {
            cmd_fork(&product).await
        }
        Commands::Search {
            query,
            category,
            sort,
            limit,
        } => {
            cmd_search(&query, category.as_deref(), &sort, limit).await
        }
        Commands::Init { slug } => {
            cmd_init(slug)
        }
        Commands::Products { status } => {
            cmd_products(status).await
        }
        Commands::Status => {
            cmd_status()
        }
        Commands::Upstream => {
            cmd_upstream().await
        }
    };

    // Print update notice if available (non-blocking, 100ms timeout)
    if let Ok(Ok(Some(notice))) =
        tokio::time::timeout(std::time::Duration::from_millis(100), update_handle).await
    {
        eprintln!("{}", notice);
    }

    result
}

fn read_readme(dir: &std::path::Path) -> Option<String> {
    for name in &["README.md", "readme.md", "Readme.md", "README", "README.txt"] {
        let path = dir.join(name);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if !content.trim().is_empty() {
                return Some(content);
            }
        }
    }
    None
}

const STARTING_VERSIONS: &[&str] = &["0.0.1", "0.1.0", "1.0.0"];

struct PublishContext {
    slug: String,
    product_name: String,
    product_desc: Option<String>,
    category_slug: String,
    license: String,
    version: String,
    changelog_text: String,
    readme: Option<String>,
    existing_manifest: Option<types::Manifest>,
}

/// Shared publish steps: gate → package → create/find product → upload → confirm → manifest → track
async fn execute_publish(
    client: &api::BaroClient,
    namespace: &str,
    cwd: &std::path::Path,
    ctx: PublishContext,
) -> Result<()> {
    // 1. Run publish gate
    let categories = client.list_categories().await?;
    let gate = publish_gate::run(
        cwd,
        &ctx.version,
        ctx.product_desc.as_deref(),
        &ctx.category_slug,
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

    // 2. Package
    println!("Packaging...");
    let (archive_bytes, hash) = packaging::create_archive(cwd)?;
    let size = archive_bytes.len() as i64;
    println!(
        "  Archive: {} ({})",
        utils::format_bytes(size),
        &hash[..12]
    );

    // 3. Create or find product
    let my_products = client.list_my_products().await?;
    let existing_product = my_products.products.iter().find(|p| p.slug == ctx.slug);
    let product_id = if let Some(ep) = existing_product {
        ep.id.clone()
    } else {
        let desc = ctx.product_desc.as_ref().ok_or_else(|| anyhow::anyhow!(
            "Description required (50+ chars) for first publish. Use --description or add to your Cargo.toml/package.json."
        ))?;
        println!("Creating product {}/{}...", namespace, ctx.slug);
        let created = client
            .create_product(&ctx.slug, &ctx.product_name, desc, &ctx.category_slug, &ctx.license)
            .await?;
        created.product.id.clone()
    };

    // 4. Create release
    println!("Uploading v{}...", ctx.version);
    let release = client
        .create_release(namespace, &ctx.slug, &ctx.version, &ctx.changelog_text, size, &hash, ctx.readme.as_deref())
        .await?;

    // 5. Upload to R2
    client
        .upload_to_r2(&release.upload_url, &archive_bytes)
        .await?;

    // 6. Confirm
    let confirm = client.confirm_release(&release.release_id).await?;

    println!(
        "\nPublished {}/{}@{} ({})",
        namespace, ctx.slug, ctx.version,
        utils::format_bytes(size)
    );
    match confirm.review_status.as_deref() {
        Some("published") => println!("Status: published"),
        Some("unlisted") => println!("Status: unlisted (not visible in browse)"),
        Some("pending_review") => println!("Status: pending_review (admin approval required)"),
        Some(s) => println!("Status: {}", s),
        None => println!("Status: pending_review (admin approval required)"),
    }

    // 7. Write/update manifest
    let updated_manifest = types::Manifest {
        origin: ctx.existing_manifest.as_ref().and_then(|m| m.origin.clone()),
        cloned_at: ctx.existing_manifest.as_ref().and_then(|m| m.cloned_at.clone()),
        file_hash: ctx.existing_manifest.as_ref().and_then(|m| m.file_hash.clone()),
        slug: Some(ctx.slug.clone()),
        product_id: Some(product_id.clone()),
        publisher: Some(namespace.to_string()),
        version: ctx.version.clone(),
    };
    manifest::write(cwd, &updated_manifest)?;

    // 8. Track remake if this is a forked product
    if let Some(ref origin) = updated_manifest.origin {
        let origin_parts: Vec<&str> = origin.splitn(2, '/').collect();
        if origin_parts.len() == 2 {
            match client
                .track_remake(origin_parts[0], origin_parts[1], &product_id, &updated_manifest.version)
                .await
            {
                Ok(_) => println!("Remake tracked from {}", origin),
                Err(e) => eprintln!("Warning: could not track fork: {}", e),
            }
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

    // 2. Read manifest for product identity
    let cwd = std::env::current_dir()?;
    let existing_manifest = manifest::read(&cwd).ok();

    // Block publish on unpublished forks — direct to remake
    if let Some(ref m) = existing_manifest {
        if m.origin.is_some() && m.product_id.is_none() {
            return Err(anyhow::anyhow!(
                "This is a forked product (from {}). Use `baro remake` to publish it as your own.\n\
                Example: baro remake --version {} --slug <your-slug> --category <category>",
                m.origin.as_deref().unwrap_or("unknown"),
                version
            ));
        }
    }

    let slug = match &existing_manifest {
        Some(m) if m.slug.is_some() => m.slug.clone().unwrap(),
        _ => {
            // No manifest or no slug in manifest
            if !STARTING_VERSIONS.contains(&version.as_str()) {
                return Err(anyhow::anyhow!(
                    "No .baro/manifest.json found. This looks like an existing product.\n\
                    Run `baro init` first, or use a starting version (0.0.1, 0.1.0, 1.0.0)."
                ));
            }
            // Auto-init for starting versions
            let derived_slug = utils::dir_to_slug(&cwd);
            if !validate_slug(&derived_slug) {
                return Err(anyhow::anyhow!(
                    "Directory name '{}' is not a valid slug. Run `baro init --slug <slug>` first.",
                    derived_slug
                ));
            }
            derived_slug
        }
    };

    // 3. Extract metadata from build files or flags
    let (detected_name, detected_desc) = utils::detect_metadata(&cwd);
    let product_name = name_flag
        .or(detected_name)
        .unwrap_or_else(|| slug.clone());
    let product_desc = description_flag.or(detected_desc);

    // 4. Resolve category
    let category_slug = match &category {
        Some(c) => c.clone(),
        None => {
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

    // 5. Resolve changelog
    let changelog_text = match changelog {
        Some(cl) => cl,
        None => utils::read_changelog(&cwd, &version)
            .unwrap_or_else(|| format!("Release {}", version)),
    };

    // 6. Read README for product page
    let readme = read_readme(&cwd);

    execute_publish(&client, &me.user.username, &cwd, PublishContext {
        slug,
        product_name,
        product_desc,
        category_slug,
        license,
        version,
        changelog_text,
        readme,
        existing_manifest,
    }).await
}

async fn cmd_remake(
    version: String,
    slug_flag: Option<String>,
    changelog: Option<String>,
    category: String,
    name_flag: Option<String>,
    description_flag: Option<String>,
    license: String,
) -> Result<()> {
    let token = auth::get_token().await?;
    let client = api::BaroClient::new(&token);

    // 1. Get publisher info
    let me = client.get_me().await?;
    println!("Remaking as {}...", me.user.username);

    // 2. Read manifest — require fork origin
    let cwd = std::env::current_dir()?;
    let existing_manifest = manifest::read(&cwd).ok();

    let manifest = existing_manifest.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No .baro/manifest.json found. This is not a forked product.\nUse `baro publish` for your own products.")
    })?;

    let origin = manifest.origin.as_deref().ok_or_else(|| {
        anyhow::anyhow!("No fork origin in manifest. This is not a forked product.\nUse `baro publish` for your own products.")
    })?;

    // 3. Block if already remade
    if manifest.product_id.is_some() {
        let slug = manifest.slug.as_deref().unwrap_or("?");
        let publisher = manifest.publisher.as_deref().unwrap_or("?");
        return Err(anyhow::anyhow!(
            "Already remade as {}/{}. Use `baro publish` for new versions.\n\
            Example: baro publish --version {}",
            publisher, slug, version
        ));
    }

    // 4. Resolve slug
    let slug = slug_flag.unwrap_or_else(|| utils::dir_to_slug(&cwd));
    if !validate_slug(&slug) {
        return Err(anyhow::anyhow!(
            "Invalid slug '{}'. Must be lowercase alphanumeric with hyphens, not starting/ending with hyphen.",
            slug
        ));
    }

    // 5. Check for slug collision with own products
    let my_products = client.list_my_products().await?;
    if my_products.products.iter().any(|p| p.slug == slug) {
        return Err(anyhow::anyhow!(
            "Slug '{}' is already used by your product. Use --slug <different-name> to pick a new one.\n\
            Example: baro remake --version {} --slug {}-remix --category {}",
            slug, version, slug, category
        ));
    }

    // 6. Self-fork confirmation
    let origin_parts: Vec<&str> = origin.splitn(2, '/').collect();
    if origin_parts.len() == 2 && origin_parts[0] == me.user.username {
        eprint!("You're remaking your own product ({}). Continue? [Y/n] ", origin);
        std::io::Write::flush(&mut std::io::stderr())?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        if !input.is_empty() && input != "y" && input != "yes" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // 7. Extract metadata
    let (detected_name, detected_desc) = utils::detect_metadata(&cwd);
    let product_name = name_flag
        .or(detected_name)
        .unwrap_or_else(|| slug.clone());
    let product_desc = description_flag.or(detected_desc);

    // 8. Resolve changelog
    let changelog_text = match changelog {
        Some(cl) => cl,
        None => utils::read_changelog(&cwd, &version)
            .unwrap_or_else(|| format!("Release {}", version)),
    };

    // 9. Read README
    let readme = read_readme(&cwd);

    println!("Remaking from {} → {}/{}...", origin, me.user.username, slug);

    execute_publish(&client, &me.user.username, &cwd, PublishContext {
        slug,
        product_name,
        product_desc,
        category_slug: category,
        license,
        version,
        changelog_text,
        readme,
        existing_manifest,
    }).await
}

async fn cmd_fork(product: &str) -> Result<()> {
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
            eprint!("Login required to fork. Open browser to sign up? [Y/n] ");
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
    println!("Forking {}/{}@{}...", username, slug, target_version);
    let download = client
        .get_download(username, slug, &target_version)
        .await?;

    // Download file from R2
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
            "Directory '{}' already exists. Remove it first or fork to a different location.",
            slug
        ));
    }
    packaging::extract_archive(&bytes, dest)?;

    // Write manifest
    let m = types::Manifest {
        origin: Some(format!("{}/{}", username, slug)),
        version: target_version.clone(),
        cloned_at: Some(chrono::Utc::now().to_rfc3339()),
        file_hash: Some(actual_hash),
        slug: None,
        product_id: None,
        publisher: None,
    };
    manifest::write(dest, &m)?;

    println!(
        "Forked {}/{}@{} → ./{}/  ({})",
        username,
        slug,
        target_version,
        slug,
        utils::format_bytes(bytes.len() as i64)
    );
    println!();
    println!("Next steps:");
    println!("  1. Read README.md for setup instructions");
    println!("  2. Build and run the project");
    println!("  3. Customize with AI — ask what to change");

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
        let desc = utils::truncate_str(&p.description, 60);

        println!("{}/{:<20} v{:<8} [{}]", pub_name, p.slug, ver, cat_name);
        println!("  {}", desc);

        if let Some(ref stats) = p.stats {
            let forks = stats.fork_count.unwrap_or(0);
            let rating = stats
                .avg_rating
                .map(|r| format!("{:.1}/5", r))
                .unwrap_or_else(|| "-".to_string());
            let rc = stats.rating_count.unwrap_or(0);
            println!("  Forks: {}  Rating: {} ({})  Updated: {}", forks, rating, rc, &p.updated_at[..10]);
        }
        println!();
    }

    println!("Found {} results (showing {})", resp.total, resp.products.len());
    Ok(())
}

fn validate_slug(slug: &str) -> bool {
    if slug.is_empty() {
        return false;
    }
    let bytes = slug.as_bytes();
    // Must start and end with alphanumeric
    if !bytes[0].is_ascii_alphanumeric() || !bytes[bytes.len() - 1].is_ascii_alphanumeric() {
        return false;
    }
    // All chars must be lowercase alphanumeric or hyphen
    slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn cmd_init(slug_flag: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;

    // Check if manifest already exists
    if let Ok(m) = manifest::read(&cwd) {
        let slug = m.slug.as_deref().unwrap_or("(not set)");
        let publisher = m.publisher.as_deref().unwrap_or("(not published yet)");
        println!("Already initialized:");
        println!("  Slug:      {}", slug);
        println!("  Publisher: {}", publisher);
        println!("  Version:   {}", m.version);
        return Ok(());
    }

    // Derive slug
    let slug = slug_flag.unwrap_or_else(|| utils::dir_to_slug(&cwd));

    if !validate_slug(&slug) {
        return Err(anyhow::anyhow!(
            "Invalid slug '{}'. Must be lowercase alphanumeric with hyphens, not starting/ending with hyphen.",
            slug
        ));
    }

    // Write manifest
    let m = types::Manifest {
        origin: None,
        cloned_at: None,
        file_hash: None,
        slug: Some(slug.clone()),
        product_id: None,
        publisher: None,
        version: "0.0.0".to_string(),
    };
    manifest::write(&cwd, &m)?;

    println!("Initialized baro product: {}", slug);
    println!("  Manifest: .baro/manifest.json");
    Ok(())
}

async fn cmd_products(status_filter: Option<String>) -> Result<()> {
    let token = auth::get_token().await?;
    let client = api::BaroClient::new(&token);
    let me = client.get_me().await?;
    let resp = client.list_my_products().await?;

    let products: Vec<&types::Product> = if let Some(ref status) = status_filter {
        resp.products.iter().filter(|p| p.review_status == *status).collect()
    } else {
        resp.products.iter().collect()
    };

    if products.is_empty() {
        if status_filter.is_some() {
            println!("No products with status '{}'", status_filter.unwrap());
        } else {
            println!("No products yet. Run `baro publish` to get started.");
        }
        return Ok(());
    }

    for p in &products {
        let cat_name = p.category.as_ref().map(|c| c.slug.as_str()).unwrap_or("?");
        let ver = p.latest_version.as_deref().unwrap_or("-");
        let desc = utils::truncate_str(&p.description, 60);

        println!(
            "{}/{:<20} v{:<8} [{}]  {}",
            me.user.username, p.slug, ver, cat_name, p.review_status
        );
        println!("  {}", desc);

        if let Some(ref stats) = p.stats {
            let forks = stats.fork_count.unwrap_or(0);
            let rating = stats
                .avg_rating
                .map(|r| format!("{:.1}/5", r))
                .unwrap_or_else(|| "-".to_string());
            let rc = stats.rating_count.unwrap_or(0);
            println!("  Forks: {}  Rating: {} ({})", forks, rating, rc);
        }
        println!();
    }

    println!("{} product{}", products.len(), if products.len() == 1 { "" } else { "s" });
    Ok(())
}

fn cmd_status() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let m = manifest::read(&cwd)?;

    // Show publish identity if present
    if let Some(ref slug) = m.slug {
        let publisher = m.publisher.as_deref().unwrap_or("?");
        println!("Product: {}/{}", publisher, slug);
        println!("Version: {}", m.version);
        if let Some(ref pid) = m.product_id {
            println!("ID:      {}", pid);
        }
    }

    // Show fork origin if present
    if let Some(ref origin) = m.origin {
        println!("Origin:  {}", origin);
        if let Some(ref cloned_at) = m.cloned_at {
            println!("Forked:  {}", cloned_at);
        }
    }

    // Fallback: if neither publish nor fork info
    if m.slug.is_none() && m.origin.is_none() {
        println!("Version: {}", m.version);
    }

    Ok(())
}

async fn cmd_upstream() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let m = manifest::read(&cwd)?;

    let origin = m.origin.as_deref().ok_or_else(|| {
        anyhow::anyhow!("No fork origin in manifest. This product was not forked.")
    })?;
    let parts: Vec<&str> = origin.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!("Invalid origin in manifest: {}", origin));
    }
    let (username, slug) = (parts[0], parts[1]);

    let client = api::BaroClient::anonymous();
    let releases = client.list_releases(username, slug).await?;

    match releases.releases.first() {
        Some(latest) if latest.version != m.version => {
            println!("New version available: {} (current: {})", latest.version, m.version);
            if let Some(ref cl) = latest.changelog {
                let preview = utils::truncate_str(cl, 100);
                println!("  Changelog: {}", preview);
            }
            println!("  Run: baro fork {}@{}", origin, latest.version);
        }
        Some(_) => {
            println!("Up to date with upstream ({})", m.version);
        }
        None => {
            println!("No releases found for {}", origin);
        }
    }

    Ok(())
}
