//! `ometa quality <fqn>` — show data-quality test cases for an entity.

use crate::client::OmClient;
use crate::commands::{pick, truncate};
use crate::config::{resolve, Overrides};
use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use serde_json::Value;
use tabled::settings::Style;
use tabled::{Table, Tabled};

#[derive(Parser, Debug)]
pub struct Args {
    /// Fully-qualified table name.
    pub fqn: String,
    /// Print raw JSON instead of a formatted table.
    #[arg(long)]
    pub json: bool,
}

#[derive(Tabled)]
struct Row {
    #[tabled(rename = "Test")]
    test: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Last run")]
    last_run: String,
    #[tabled(rename = "Failure")]
    failure: String,
}

pub fn run(args: Args, overrides: Overrides) -> Result<()> {
    let cfg = resolve(overrides)?;
    let client = OmClient::new(cfg.host, cfg.token)?;
    let raw = client.quality_for(&args.fqn)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&raw)?);
        return Ok(());
    }

    let mut rows: Vec<Row> = Vec::new();
    if let Some(suites) = raw.get("data").and_then(Value::as_array) {
        for suite in suites {
            if let Some(tests) = suite.get("tests").and_then(Value::as_array) {
                for test in tests {
                    rows.push(test_to_row(test));
                }
            }
        }
    }
    if rows.is_empty() {
        println!("{}", "No data-quality tests found for this entity.".yellow());
        return Ok(());
    }
    let mut table = Table::new(rows);
    table.with(Style::rounded());
    println!("{table}");
    Ok(())
}

fn test_to_row(test: &Value) -> Row {
    let status_raw = pick(
        test,
        &["testCaseStatus", "status"],
    );
    let status_pretty = match status_raw.to_lowercase().as_str() {
        "success" | "passed" | "pass" => "✅ pass".to_string(),
        "failed" | "fail" => "❌ fail".to_string(),
        "aborted" | "warning" => "⚠️  warning".to_string(),
        _ => "—".to_string(),
    };
    Row {
        test: pick(test, &["name", "displayName"]),
        status: status_pretty,
        last_run: pick(test, &["timestamp", "updatedAt"]),
        failure: truncate(&pick(test, &["result", "message", "failureReason"]), 60),
    }
}
