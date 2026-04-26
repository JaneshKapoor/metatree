//! `ometa lineage <fqn>` — render lineage as an ASCII tree.

use crate::client::OmClient;
use crate::config::{resolve, Overrides};
use anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};
use colored::Colorize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Parser, Debug)]
pub struct Args {
    /// Fully-qualified name of the table.
    pub fqn: String,
    /// Which side of the graph to walk.
    #[arg(long, default_value = "both")]
    pub direction: Direction,
    /// Maximum depth in either direction.
    #[arg(long, default_value_t = 2)]
    pub depth: u32,
    /// Print raw JSON (nodes + edges) instead of a tree.
    #[arg(long)]
    pub json: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum Direction {
    Upstream,
    Downstream,
    Both,
}

pub fn run(args: Args, overrides: Overrides) -> Result<()> {
    let cfg = resolve(overrides)?;
    let client = OmClient::new(cfg.host, cfg.token)?;
    let table = client
        .table_by_fqn(&args.fqn, "id")?
        .ok_or_else(|| anyhow!("table `{}` not found in OpenMetadata catalog", args.fqn))?;
    let id = table
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("table response did not include an `id` field"))?
        .to_string();

    let (up, down) = match args.direction {
        Direction::Upstream => (args.depth, 0),
        Direction::Downstream => (0, args.depth),
        Direction::Both => (args.depth, args.depth),
    };

    let lineage = client
        .lineage_by_id(&id, up, down)?
        .unwrap_or_else(|| serde_json::json!({"nodes": [], "upstreamEdges": [], "downstreamEdges": []}));

    if args.json {
        println!("{}", serde_json::to_string_pretty(&lineage)?);
        return Ok(());
    }
    print_tree(&id, &args.fqn, &lineage, args.direction);
    Ok(())
}

fn print_tree(root_id: &str, root_fqn: &str, lineage: &Value, dir: Direction) {
    let nodes_by_id: HashMap<String, &Value> = lineage
        .get("nodes")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|n| n.get("id").and_then(Value::as_str).map(|id| (id.to_string(), n)))
                .collect()
        })
        .unwrap_or_default();

    println!(
        "{} {}",
        format_kind("table"),
        root_fqn.bold()
    );
    let show_up = matches!(dir, Direction::Upstream | Direction::Both);
    let show_down = matches!(dir, Direction::Downstream | Direction::Both);
    if show_up {
        println!("├── upstream");
        let edges = collect_edges(lineage, "upstreamEdges", root_id, true);
        print_branch(&edges, &nodes_by_id, "│   ");
    }
    if show_down {
        println!("└── downstream");
        let edges = collect_edges(lineage, "downstreamEdges", root_id, false);
        print_branch(&edges, &nodes_by_id, "    ");
    }
}

fn collect_edges<'a>(
    lineage: &'a Value,
    key: &str,
    root_id: &str,
    upstream: bool,
) -> Vec<&'a Value> {
    let mut out = Vec::new();
    let edges = match lineage.get(key).and_then(Value::as_array) {
        Some(a) => a,
        None => return out,
    };
    for edge in edges {
        let from = edge_id(edge, "fromEntity");
        let to = edge_id(edge, "toEntity");
        let touches_root = match (upstream, &from, &to) {
            (true, _, Some(t)) if t == root_id => true,
            (false, Some(f), _) if f == root_id => true,
            _ => false,
        };
        if touches_root {
            out.push(edge);
        }
    }
    out
}

fn edge_id(edge: &Value, key: &str) -> Option<String> {
    match edge.get(key)? {
        Value::String(s) => Some(s.clone()),
        Value::Object(obj) => obj.get("id").and_then(Value::as_str).map(|s| s.to_string()),
        _ => None,
    }
}

fn print_branch(edges: &[&Value], nodes: &HashMap<String, &Value>, indent: &str) {
    if edges.is_empty() {
        println!("{indent}└── (none)");
        return;
    }
    for (i, edge) in edges.iter().enumerate() {
        let last = i == edges.len() - 1;
        let bullet = if last { "└──" } else { "├──" };
        let target = edge_id(edge, "toEntity")
            .or_else(|| edge_id(edge, "fromEntity"))
            .unwrap_or_default();
        let node = nodes.get(&target);
        let name = node
            .and_then(|n| n.get("fullyQualifiedName").and_then(Value::as_str))
            .or_else(|| node.and_then(|n| n.get("name").and_then(Value::as_str)))
            .unwrap_or("(unknown)");
        let kind = node
            .and_then(|n| n.get("type").and_then(Value::as_str))
            .or_else(|| node.and_then(|n| n.get("entityType").and_then(Value::as_str)))
            .unwrap_or("table");
        println!("{indent}{bullet} {} {}", format_kind(kind), name);
    }
}

fn format_kind(kind: &str) -> colored::ColoredString {
    match kind.to_lowercase().as_str() {
        "dashboard" => "[dashboard]".magenta(),
        "pipeline" => "[pipeline]".cyan(),
        "topic" => "[topic]".yellow(),
        _ => "[table]".blue(),
    }
}
