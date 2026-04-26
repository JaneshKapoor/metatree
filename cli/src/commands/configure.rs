//! `ometa configure` — interactive (or flag-driven) setup wizard.

use crate::config::{save_disk, OnDisk, Overrides};
use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use std::io::{self, Write};

#[derive(Parser, Debug)]
pub struct Args {
    /// Set OPENMETADATA_HOST without prompting.
    #[arg(long)]
    pub host: Option<String>,
    /// Set OPENMETADATA_JWT_TOKEN without prompting.
    #[arg(long)]
    pub token: Option<String>,
}

pub fn run(args: Args, _: Overrides) -> Result<()> {
    let host = match args.host {
        Some(h) => h,
        None => prompt(
            "OpenMetadata host (e.g. https://sandbox.open-metadata.org/api): ",
            false,
        )?,
    };
    let token = match args.token {
        Some(t) => t,
        None => prompt(
            "JWT token (Settings -> Bots -> ingestion-bot -> JWT Token): ",
            true,
        )?,
    };
    let on_disk = OnDisk {
        host: Some(host.trim_end_matches('/').to_string()),
        token: Some(token),
    };
    let path = save_disk(&on_disk)?;
    println!("{} {}", "Saved:".green().bold(), path.display());
    Ok(())
}

fn prompt(label: &str, _secret: bool) -> Result<String> {
    print!("{label}");
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}
