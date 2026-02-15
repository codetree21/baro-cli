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

        /// Publish as a team product
        #[arg(long)]
        team: Option<String>,
    },

    /// Clone a product (download + unpack)
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

    /// Show fork origin and version info
    Status,

    /// Check for new releases from fork origin
    Upstream,

    /// Team management
    Team {
        #[command(subcommand)]
        action: TeamCommands,
    },
}

#[derive(Subcommand)]
pub enum TeamCommands {
    /// Create a new team
    Create {
        /// Team name (lowercase alphanumeric with hyphens)
        name: String,
        /// Display name
        #[arg(long)]
        display_name: Option<String>,
    },
    /// List your teams and pending invitations
    List,
    /// Show team details and members
    Info {
        /// Team name
        name: String,
    },
    /// Invite a user to a team (owner only)
    Invite {
        /// Team name
        team: String,
        /// Username to invite
        username: String,
    },
    /// Accept a team invitation
    Accept {
        /// Invitation ID
        invitation_id: String,
    },
    /// Reject a team invitation
    Reject {
        /// Invitation ID
        invitation_id: String,
    },
    /// Remove a member from a team
    Remove {
        /// Team name
        team: String,
        /// Username to remove
        username: String,
    },
}
