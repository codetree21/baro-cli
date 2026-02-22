use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "baro", about = "CLI for the Baro AI product marketplace")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Authenticate with GitHub OAuth
    Login,

    /// Publish a product release (package + validate + upload)
    Publish {
        /// Version string (e.g., "1.0.0")
        #[arg(long)]
        version: String,

        /// Changelog describing what changed and why
        #[arg(long)]
        changelog: Option<String>,

        /// Category slug (e.g., developer-tools, productivity, ai-agents)
        #[arg(long)]
        category: Option<String>,

        /// Product display name (default: from build file or directory name)
        #[arg(long)]
        name: Option<String>,

        /// Product description, 50+ chars (default: from build file)
        #[arg(long)]
        description: Option<String>,

        /// License identifier (default: MIT)
        #[arg(long, default_value = "MIT")]
        license: String,

    },

    /// Fork a product (download + unpack)
    Fork {
        /// Product identifier: user/product or user/product@version
        product: String,
    },

    /// Alias for fork (hidden)
    #[command(hide = true)]
    Clone {
        /// Product identifier: user/product or user/product@version
        product: String,
    },

    /// Search for products
    Search {
        /// Search query
        query: String,

        /// Filter by category slug
        #[arg(long)]
        category: Option<String>,

        /// Sort order: recent, downloads, rating
        #[arg(long, default_value = "recent")]
        sort: String,

        /// Max results to show
        #[arg(long, default_value = "20")]
        limit: u32,
    },

    /// Initialize a baro product in the current directory
    Init {
        /// Product slug (default: derived from directory name)
        #[arg(long)]
        slug: Option<String>,
    },

    /// List your published products
    Products {
        /// Filter by status: published, pending_review, unlisted, rejected
        #[arg(long)]
        status: Option<String>,
    },

    /// Show product identity and fork origin info
    Status,

    /// Check for new releases from fork origin
    Upstream,
}
