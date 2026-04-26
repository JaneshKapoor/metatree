pub mod configure;
pub mod describe;
pub mod lineage;
pub mod mcp;
pub mod patch;
pub mod quality;
pub mod search;

use serde_json::Value;

/// Pull a string from a JSON value, falling back through a list of keys, returning "—" when absent.
pub fn pick(v: &Value, keys: &[&str]) -> String {
    for k in keys {
        if let Some(s) = v.get(*k).and_then(|x| x.as_str()) {
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    "—".to_string()
}

/// Format an owner field which may be missing, a single object, or a list.
pub fn pick_owner(v: &Value) -> String {
    if let Some(arr) = v.get("owners").and_then(|x| x.as_array()) {
        if let Some(first) = arr.first() {
            return pick(first, &["displayName", "name"]);
        }
    }
    if let Some(o) = v.get("owner") {
        return pick(o, &["displayName", "name"]);
    }
    "—".to_string()
}

/// Truncate a string to `n` characters, appending an ellipsis if it was cut.
pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
    out.push('…');
    out
}
