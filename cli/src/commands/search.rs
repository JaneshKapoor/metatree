//! `ometa search <query>` — search the OpenMetadata catalog.

use crate::client::OmClient;
use crate::commands::{pick, pick_owner, truncate};
use crate::config::{resolve, Overrides};
use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use serde_json::Value;
use tabled::settings::Style;
use tabled::{Table, Tabled};

#[derive(Parser, Debug)]
pub struct Args {
    /// The search term (table / dashboard / pipeline / column).
    pub query: String,
    /// Limit to a single entity type, or `all`.
    #[arg(long, default_value = "table")]
    pub r#type: String,
    /// Maximum number of hits to return.
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
    /// Print raw JSON instead of a formatted table.
    #[arg(long)]
    pub json: bool,
}

#[derive(Tabled)]
struct Row {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    kind: String,
    #[tabled(rename = "Owner")]
    owner: String,
    #[tabled(rename = "Description")]
    description: String,
    #[tabled(rename = "Score")]
    score: String,
}

pub fn run(args: Args, overrides: Overrides) -> Result<()> {
    let cfg = resolve(overrides)?;
    let client = OmClient::new(cfg.host, cfg.token)?;
    let index = match args.r#type.as_str() {
        "all" => "all",
        "dashboard" => "dashboard_search_index",
        "pipeline" => "pipeline_search_index",
        _ => "table_search_index",
    };
    let raw = client.search(&args.query, index, args.limit)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&raw)?);
        return Ok(());
    }
    let hits = raw
        .pointer("/hits/hits")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if hits.is_empty() {
        println!("{}", "No results.".yellow());
        return Ok(());
    }
    let rows: Vec<Row> = hits.iter().map(hit_to_row).collect();
    let mut table = Table::new(rows);
    table.with(Style::rounded());
    println!("{table}");
    Ok(())
}

fn hit_to_row(hit: &Value) -> Row {
    let src = hit.get("_source").cloned().unwrap_or(Value::Null);
    let score = hit
        .get("_score")
        .and_then(Value::as_f64)
        .map(|f| format!("{f:.2}"))
        .unwrap_or_else(|| "—".to_string());
    Row {
        name: pick(&src, &["fullyQualifiedName", "name", "displayName"]),
        kind: pick(&src, &["entityType", "type"]),
        owner: pick_owner(&src),
        description: truncate(&pick(&src, &["description"]), 60),
        score,
    }
}
