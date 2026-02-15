# Baro CLI

Command-line interface for the [Baro](https://baro-sync.com) AI product marketplace. Clone, customize with AI, and re-publish.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/codetree21/baro-cli/main/install.sh | sh
```

Or build from source:

```bash
git clone https://github.com/codetree21/baro-cli.git
cd baro-cli
cargo build --release
cp target/release/baro ~/.local/bin/
```

## Quick Start

```bash
# Authenticate
baro login

# Clone a product
baro clone acme/invoice-tool

# Customize with any AI tool, then publish
baro publish --version 1.0.0
```

## Commands

| Command | Description |
|---------|-------------|
| `baro login` | Authenticate via GitHub OAuth |
| `baro publish --version <ver>` | Publish a product release |
| `baro clone <user/product>` | Clone a product (supports `@version`) |
| `baro search <query>` | Search for products |
| `baro status` | Show fork origin and version info |
| `baro upstream` | Check for new releases from fork origin |
| `baro team create <name>` | Create a new team |
| `baro team list` | List your teams and invitations |
| `baro team info <name>` | Show team details and members |
| `baro team invite <team> <user>` | Invite a user to a team |
| `baro team accept <id>` | Accept a team invitation |
| `baro team reject <id>` | Reject a team invitation |
| `baro team remove <team> <user>` | Remove a member from a team |

## Publish Options

```bash
baro publish --version 1.0.0 \
  --category developer-tools \
  --license MIT \
  --changelog "Added dark mode" \
  --team my-team
```

## Configuration

Environment variable overrides (defaults are embedded):

| Variable | Description |
|----------|-------------|
| `BARO_API_URL` | API base URL (default: https://baro-sync.com) |
| `BARO_SUPABASE_URL` | Supabase project URL |
| `BARO_SUPABASE_ANON_KEY` | Supabase anonymous key |

## Documentation

Full documentation: https://baro-sync.com/docs
