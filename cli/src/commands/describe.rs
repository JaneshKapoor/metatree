//! `ometa describe <fqn>` — full details for a single entity.

use crate::client::OmClient;
use crate::commands::{pick, pick_owner, truncate};
use crate::config::{resolve, Overrides};
use anyhow::{anyhow, Result};
use clap::Parser;
use colored::Colorize;
use serde_json::Value;
use tabled::settings::Style;
use tabled::{Table, Tabled};

#[derive(Parser, Debug)]
pub struct Args {
    /// Fully-qualified name, e.g. `sample_data.ecommerce_db.shopify.dim_customer`.
    pub fqn: String,
    /// Print raw JSON instead of a formatted view.
    #[arg(long)]
    pub json: bool,
}

#[derive(Tabled)]
struct ColumnRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    data_type: String,
    #[tabled(rename = "Nullable")]
    nullable: String,
    #[tabled(rename = "Tags")]
    tags: String,
    #[tabled(rename = "Description")]
    description: String,
}

pub fn run(args: Args, overrides: Overrides) -> Result<()> {
    let cfg = resolve(overrides)?;
    let client = OmClient::new(cfg.host, cfg.token)?;
    let table_value = client
        .table_by_fqn(&args.fqn, "owners,tags,columns,testSuite,customProperties")?
        .ok_or_else(|| anyhow!("table `{}` not found in OpenMetadata catalog", args.fqn))?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&table_value)?);
        return Ok(());
    }
    render_human(&table_value);
    Ok(())
}

fn render_human(t: &Value) {
    println!("{} {}", "📊".to_string(), pick(t, &["fullyQualifiedName", "name"]).bold());
    println!("  {} {}", "Type:".dimmed(), pick(t, &["entityType", "type"]));
    println!("  {} {}", "Owner:".dimmed(), pick_owner(t));
    println!("  {} {}", "Description:".dimmed(), pick(t, &["description"]));

    if let Some(tags) = t.get("tags").and_then(Value::as_array) {
        if !tags.is_empty() {
            let names: Vec<String> = tags
                .iter()
                .filter_map(|tag| tag.get("tagFQN").and_then(Value::as_str))
                .map(|s| s.to_string())
                .collect();
            if !names.is_empty() {
                println!("  {} {}", "Tags:".dimmed(), names.join(", ").yellow());
            }
        }
    }

    let columns = t
        .get("columns")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !columns.is_empty() {
        println!("\n{}", "Columns".bold());
        let rows: Vec<ColumnRow> = columns.iter().map(column_row).collect();
        let mut table = Table::new(rows);
        table.with(Style::rounded());
        println!("{table}");
    }

    if let Some(suite) = t.get("testSuite") {
        if !suite.is_null() {
            let summary = pick(suite, &["name", "fullyQualifiedName"]);
            println!("\n{} {}", "Data quality:".dimmed(), summary);
        }
    }
}

fn column_row(c: &Value) -> ColumnRow {
    let nullable = match c.get("constraint").and_then(Value::as_str) {
        Some("NOT_NULL") | Some("PRIMARY_KEY") => "no",
        _ => "yes",
    };
    let tags = c
        .get("tags")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.get("tagFQN").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    ColumnRow {
        name: pick(c, &["name"]),
        data_type: pick(c, &["dataType", "type"]),
        nullable: nullable.to_string(),
        tags,
        description: truncate(&pick(c, &["description"]), 50),
    }
}
