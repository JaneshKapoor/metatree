//! `ometa patch <fqn>` — patch fields on an entity.
//!
//! Builds a JSON-Patch (RFC 6902) body and PATCHes `/api/v1/tables/name/{fqn}`.

use crate::client::OmClient;
use crate::commands::pick;
use crate::config::{resolve, Overrides};
use anyhow::{anyhow, Result};
use clap::Parser;
use colored::Colorize;
use serde_json::{json, Value};
use std::io::{self, Write};

#[derive(Parser, Debug)]
pub struct Args {
    pub fqn: String,
    /// New description for the table.
    #[arg(long)]
    pub description: Option<String>,
    /// Owner team / user name to set.
    #[arg(long)]
    pub owner: Option<String>,
    /// Tag FQN to add (e.g. `PII.Sensitive`). Repeatable.
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    /// Skip the confirmation prompt.
    #[arg(long, short = 'y')]
    pub yes: bool,
    /// Print raw JSON of the response.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args, overrides: Overrides) -> Result<()> {
    if args.description.is_none() && args.owner.is_none() && args.tags.is_empty() {
        return Err(anyhow!(
            "nothing to patch -- pass --description, --owner, or --tag"
        ));
    }
    let cfg = resolve(overrides)?;
    let client = OmClient::new(cfg.host, cfg.token)?;

    let mut ops: Vec<Value> = Vec::new();
    if let Some(desc) = &args.description {
        ops.push(json!({"op": "add", "path": "/description", "value": desc}));
    }
    if let Some(owner) = &args.owner {
        ops.push(json!({
            "op": "add",
            "path": "/owners",
            "value": [{ "type": "team", "name": owner }],
        }));
    }
    for tag in &args.tags {
        ops.push(json!({
            "op": "add",
            "path": "/tags/-",
            "value": { "tagFQN": tag, "labelType": "Manual", "state": "Confirmed" },
        }));
    }

    if !args.yes {
        println!("About to patch {}:", args.fqn.bold());
        println!("{}", serde_json::to_string_pretty(&ops)?);
        print!("Proceed? [y/N] ");
        io::stdout().flush()?;
        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        if !matches!(buf.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("{}", "Aborted.".yellow());
            return Ok(());
        }
    }

    let path = format!(
        "/api/v1/tables/name/{}",
        urlencode_fqn(&args.fqn)
    );
    let updated = client
        .patch_json::<Value>(&path, &Value::Array(ops))?
        .ok_or_else(|| anyhow!("table `{}` not found", args.fqn))?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&updated)?);
    } else {
        println!("{} {}", "Patched:".green().bold(), pick(&updated, &["fullyQualifiedName", "name"]));
        if let Some(d) = updated.get("description").and_then(Value::as_str) {
            println!("  description: {d}");
        }
    }
    Ok(())
}

fn urlencode_fqn(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(c),
            _ => {
                let mut buf = [0u8; 4];
                for &b in c.encode_utf8(&mut buf).as_bytes() {
                    out.push_str(&format!("%{:02X}", b));
                }
            }
        }
    }
    out
}
