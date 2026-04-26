//! Thin blocking REST client for the OpenMetadata API.
//!
//! Maps HTTP failures to actionable error messages:
//!   401 -> "check your JWT token"
//!   404 -> Ok(None)
//!   429 -> retry up to 3 times honoring Retry-After
//!   5xx -> retry up to 3 times, then "check your host URL"

use anyhow::{anyhow, bail, Result};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::thread;
use std::time::Duration;

pub struct OmClient {
    pub host: String,
    http: Client,
}

impl OmClient {
    pub fn new(host: impl Into<String>, token: impl AsRef<str>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        let bearer = format!("Bearer {}", token.as_ref());
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&bearer).map_err(|_| anyhow!("invalid token characters"))?,
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let http = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            host: host.into().trim_end_matches('/').to_string(),
            http,
        })
    }

    fn url(&self, path: &str) -> String {
        if path.starts_with("/api") {
            format!("{}{}", self.host.trim_end_matches("/api"), path)
        } else if path.starts_with('/') {
            format!("{}{}", self.host, path)
        } else {
            format!("{}/{}", self.host, path)
        }
    }

    fn request<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&Value>,
    ) -> Result<Option<T>> {
        let url = self.url(path);
        for attempt in 0..3 {
            let mut req = self.http.request(method.clone(), &url);
            if !query.is_empty() {
                req = req.query(query);
            }
            if let Some(b) = body {
                req = req.json(b);
            }
            let resp = req.send().map_err(|e| {
                anyhow!(
                    "could not reach OpenMetadata at {url}: {e}. Try `ometa configure` to verify your host."
                )
            })?;
            match resp.status() {
                s if s.is_success() => {
                    return resp
                        .json::<T>()
                        .map(Some)
                        .map_err(|e| anyhow!("decoding response from {url}: {e}"));
                }
                StatusCode::NOT_FOUND => return Ok(None),
                StatusCode::UNAUTHORIZED => {
                    bail!(
                        "401 Unauthorized from {url}. Check OPENMETADATA_JWT_TOKEN \
                         (Settings -> Bots -> ingestion-bot)."
                    );
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    let wait = resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(1u64 << attempt);
                    thread::sleep(Duration::from_secs(wait.max(1)));
                    continue;
                }
                s if s.is_server_error() => {
                    if attempt < 2 {
                        thread::sleep(Duration::from_secs(1u64 << attempt));
                        continue;
                    }
                    let body = resp.text().unwrap_or_default();
                    bail!(
                        "{s} from {url}. Check OPENMETADATA_HOST or service health.\n{}",
                        body.chars().take(300).collect::<String>()
                    );
                }
                s => {
                    let body = resp.text().unwrap_or_default();
                    bail!(
                        "{s} from {url}: {}",
                        body.chars().take(300).collect::<String>()
                    );
                }
            }
        }
        bail!("exceeded retries calling {url}")
    }

    pub fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Option<T>> {
        self.request::<T>(reqwest::Method::GET, path, query, None)
    }

    pub fn patch_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &Value,
    ) -> Result<Option<T>> {
        self.request::<T>(reqwest::Method::PATCH, path, &[], Some(body))
    }

    /// Convenience for `/api/v1/search/query`.
    pub fn search(
        &self,
        query: &str,
        index: &str,
        limit: usize,
    ) -> Result<Value> {
        let qs = vec![
            ("q", query.to_string()),
            ("index", index.to_string()),
            ("limit", limit.to_string()),
        ];
        Ok(self
            .get_json::<Value>("/api/v1/search/query", &qs)?
            .unwrap_or_else(|| serde_json::json!({"hits": {"hits": []}})))
    }

    /// Convenience for `/api/v1/tables/name/{fqn}` with a configurable field set.
    pub fn table_by_fqn(&self, fqn: &str, fields: &str) -> Result<Option<Value>> {
        let path = format!("/api/v1/tables/name/{}", urlencode(fqn));
        let qs = vec![("fields", fields.to_string())];
        self.get_json::<Value>(&path, &qs)
    }

    pub fn lineage_by_id(
        &self,
        id: &str,
        upstream: u32,
        downstream: u32,
    ) -> Result<Option<Value>> {
        let path = format!("/api/v1/lineage/table/{}", id);
        let qs = vec![
            ("upstreamDepth", upstream.to_string()),
            ("downstreamDepth", downstream.to_string()),
        ];
        self.get_json::<Value>(&path, &qs)
    }

    pub fn quality_for(&self, fqn: &str) -> Result<Value> {
        let qs = vec![
            ("entityLink", format!("<#E::table::{}>", fqn)),
            ("fields", "tests,testCaseResults".to_string()),
        ];
        Ok(self
            .get_json::<Value>("/api/v1/dataQuality/testSuites", &qs)?
            .unwrap_or_else(|| serde_json::json!({"data": []})))
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' | ':' | '/' => out.push(c),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_handles_leading_slash() {
        let c = OmClient::new("https://h/api", "t").unwrap();
        assert_eq!(c.url("/api/v1/x"), "https://h/api/v1/x");
        assert_eq!(c.url("/v1/x"), "https://h/api/v1/x");
        assert_eq!(c.url("v1/x"), "https://h/api/v1/x");
    }

    #[test]
    fn urlencode_keeps_safe_chars() {
        assert_eq!(urlencode("a.b.c"), "a.b.c");
        assert_eq!(urlencode("a b"), "a%20b");
    }

    #[test]
    fn search_returns_empty_on_404() {
        let mut server = mockito::Server::new();
        let url = server.url();
        let _m = server
            .mock("GET", "/api/v1/search/query")
            .match_query(mockito::Matcher::Any)
            .with_status(404)
            .create();
        let c = OmClient::new(format!("{url}/api"), "t").unwrap();
        let v = c.search("x", "table_search_index", 5).unwrap();
        assert_eq!(v["hits"]["hits"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn search_unauthorized_yields_clear_error() {
        let mut server = mockito::Server::new();
        let url = server.url();
        let _m = server
            .mock("GET", "/api/v1/search/query")
            .match_query(mockito::Matcher::Any)
            .with_status(401)
            .create();
        let c = OmClient::new(format!("{url}/api"), "t").unwrap();
        let err = c.search("x", "table_search_index", 5).unwrap_err();
        assert!(format!("{err}").contains("OPENMETADATA_JWT_TOKEN"));
    }
}
