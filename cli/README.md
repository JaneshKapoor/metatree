# `ometa` — MetaTree CLI for OpenMetadata

A small, fast Rust CLI that puts the catalog at your fingertips. Designed to be **complementary** to the official [`ai-sdk` CLI](https://github.com/open-metadata/ai-sdk/tree/main/cli) — it reuses the same env-var conventions and config-file layout (under `~/.ometa/` instead of `~/.ai-sdk/`) so you can run both side-by-side.

## Install

```bash
git clone https://github.com/your-org/metatree
cd metatree/cli
cargo install --path .
```

The binary is `ometa`, installed to `~/.cargo/bin/ometa`.

## Configure

```bash
ometa configure
# or non-interactively:
ometa configure --host https://sandbox.open-metadata.org/api --token <jwt>
```

Config is resolved with the priority **CLI flag > env var > `~/.ometa/config.toml`**. Set `OPENMETADATA_HOST` and `OPENMETADATA_JWT_TOKEN` instead of the file when you're in CI.

## Commands

| Command                                                    | What it does                                   |
| ---------------------------------------------------------- | ---------------------------------------------- |
| `ometa search <query> [--type table\|dashboard\|pipeline\|all] [--limit N] [--json]` | Search the catalog       |
| `ometa describe <fqn> [--json]`                            | Full details for one entity                    |
| `ometa lineage  <fqn> [--direction up\|down\|both] [--depth N] [--json]` | ASCII lineage tree              |
| `ometa quality  <fqn> [--json]`                            | DQ test cases and statuses                     |
| `ometa patch    <fqn> --description "..." [--owner T] [--tag PII.Sensitive] [-y]` | Patch metadata           |
| `ometa mcp [--port 3000]`                                  | Local MCP proxy for AI tools                   |

### `ometa mcp`

Starts a local HTTP server that forwards every MCP request to `{host}/mcp` with the JWT injected, so any MCP-aware client (Claude Desktop, Cursor, etc.) can use OpenMetadata without juggling tokens. Sample client config:

```json
{ "url": "http://localhost:3000/mcp" }
```

## Error handling

| HTTP | Behavior                                                            |
| ---- | ------------------------------------------------------------------- |
| 401  | "Check OPENMETADATA_JWT_TOKEN (Settings → Bots → ingestion-bot)"    |
| 404  | Treated as "not in catalog" by callers                              |
| 429  | Honors `Retry-After`, retries up to 3 times with exponential backoff|
| 5xx  | Retries up to 3 times, then "Check OPENMETADATA_HOST or service health" |

## Test

```bash
cargo test --quiet
```

Tests use `mockito` so no live OpenMetadata host is required.

## Build a release binary

```bash
cargo build --release
ls target/release/ometa
```
