#![deny(warnings)]
//! `ometa` — the MetaTree CLI for OpenMetadata.
//!
//! Config is resolved with the priority: CLI flag > env var > `~/.ometa/config.toml`.
//! All commands degrade gracefully on network errors and surface 401/404/429/5xx
//! with actionable messages.

mod client;
mod config;
mod commands;
mod spec;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "ometa",
    version,
    about = "MetaTree CLI for OpenMetadata",
    long_about = "Search, describe, walk lineage, inspect data-quality tests, patch \
                  metadata, or expose OpenMetadata's MCP server locally."
)]
struct Cli {
    /// Override OPENMETADATA_HOST (or `~/.ometa/config.toml`).
    #[arg(long, global = true)]
    host: Option<String>,

    /// Override OPENMETADATA_JWT_TOKEN (or `~/.ometa/config.toml`).
    #[arg(long, global = true)]
    token: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Interactive setup wizard. Saves to ~/.ometa/config.toml.
    Configure(commands::configure::Args),
    /// Search the OpenMetadata catalog.
    Search(commands::search::Args),
    /// Show full details for a single entity by FQN.
    Describe(commands::describe::Args),
    /// Walk upstream/downstream lineage as an ASCII tree.
    Lineage(commands::lineage::Args),
    /// Inspect data-quality test cases for an entity.
    Quality(commands::quality::Args),
    /// Patch fields on an entity (description / owner / tag).
    Patch(commands::patch::Args),
    /// Run a local MCP server proxying to {host}/mcp.
    Mcp(commands::mcp::Args),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg_overrides = config::Overrides {
        host: cli.host,
        token: cli.token,
    };

    match cli.command {
        Command::Configure(args) => commands::configure::run(args, cfg_overrides),
        Command::Search(args) => commands::search::run(args, cfg_overrides),
        Command::Describe(args) => commands::describe::run(args, cfg_overrides),
        Command::Lineage(args) => commands::lineage::run(args, cfg_overrides),
        Command::Quality(args) => commands::quality::run(args, cfg_overrides),
        Command::Patch(args) => commands::patch::run(args, cfg_overrides),
        Command::Mcp(args) => commands::mcp::run(args, cfg_overrides),
    }
}
