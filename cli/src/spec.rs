//! Helpers for fetching the OpenMetadata OpenAPI spec, used to generate
//! command stubs for endpoints that aren't covered by hand-written commands.
//! Currently a thin wrapper -- room for the SDK code-gen story to grow.

use crate::client::OmClient;
use anyhow::Result;
use serde_json::Value;

#[allow(dead_code)] // hook for future code-generated subcommands
pub fn fetch_openapi(client: &OmClient) -> Result<Option<Value>> {
    client.get_json::<Value>("/api/v1/system/version", &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_openapi_handles_missing_endpoint() {
        let mut server = mockito::Server::new();
        let url = server.url();
        let _m = server
            .mock("GET", "/api/v1/system/version")
            .with_status(404)
            .create();
        let c = OmClient::new(format!("{url}/api"), "t").unwrap();
        assert!(fetch_openapi(&c).unwrap().is_none());
    }
}
