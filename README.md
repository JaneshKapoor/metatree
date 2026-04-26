# MetaTree

> **OpenMetadata lives where developers work — in your terminal, your CI, and your editor.**

![Python](https://img.shields.io/badge/Python-3.11%2B-3776AB?logo=python&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=white)
![TypeScript](https://img.shields.io/badge/TypeScript-strict-3178C6?logo=typescript&logoColor=white)

MetaTree is a unified developer tooling suite for [OpenMetadata](https://open-metadata.org). One project, three branches that share a single backend (OpenMetadata's REST API + MCP server) and a single mental model.

```
                     +------------------------+
                     |   OpenMetadata (REST   |
                     |   + MCP server)        |
                     +-----------+------------+
                                 ^
              +------------------+------------------+
              |                  |                  |
   +----------+--------+ +-------+--------+ +-------+----------+
   | GitHub Action     | | CLI (Rust)     | | VS Code          |
   | impact analysis   | | `ometa`        | | extension        |
   | on every PR       | | search/lineage | | hover + lineage  |
   +-------------------+ +----------------+ +------------------+
              |                  |                  |
       runs in CI/CD      runs in terminal    runs while coding
```

---

## Branch 1 — GitHub Action: *"What breaks if I change this?"*

Adds an automatic impact-analysis comment to every PR that touches a `.sql`, `dbt`, or schema file. Resolves changed entity names against the OpenMetadata catalog, walks downstream lineage, and posts a Markdown report listing impacted dashboards, pipelines, and tables.

```yaml
- uses: your-org/metatree/action@v1
  with:
    openmetadata-host:  ${{ secrets.OPENMETADATA_HOST }}
    openmetadata-token: ${{ secrets.OPENMETADATA_JWT_TOKEN }}
    changed-files:      ${{ steps.changed-files.outputs.all_changed_files }}
    fail-on-impact:     false
    comment-on-pr:      true
```

See [`action/README.md`](action/README.md).

## Branch 2 — CLI: `ometa`

A fast Rust CLI that puts the catalog at your fingertips. Search, describe, walk lineage, inspect data-quality tests, patch metadata, or expose the OpenMetadata MCP server locally for AI tools.

```bash
cargo install --path cli
ometa configure
ometa search orders
ometa describe sample_data.ecommerce_db.shopify.dim_customer
ometa lineage  sample_data.ecommerce_db.shopify.dim_customer --depth 2
ometa quality  sample_data.ecommerce_db.shopify.dim_customer
ometa mcp --port 3000     # local MCP proxy for Claude Desktop / Cursor
```

See [`cli/README.md`](cli/README.md).

## Branch 3 — VS Code Extension

Surface OpenMetadata where you write SQL and dbt. Hover any table or column to see owner, tags, description, and a column list; press `Ctrl+Shift+M` to open an interactive lineage graph; the sidebar shows DQ status and failing tests for everything you've touched recently.

```bash
cd vscode-extension && npm install && npm run package
# install the resulting metatree-vscode.vsix in VS Code
```

See [`vscode-extension/README.md`](vscode-extension/README.md).

---

## Getting started in 5 minutes

You don't need to install OpenMetadata locally — the public sandbox at `https://sandbox.open-metadata.org` works out of the box.

1. **Get a sandbox JWT.** Visit <https://sandbox.open-metadata.org> → Settings → Bots → `ingestion-bot` → copy JWT Token.
2. **Set env vars** (or copy `.env.example` to `.env` and fill in):
   ```bash
   export OPENMETADATA_HOST=https://sandbox.open-metadata.org/api
   export OPENMETADATA_JWT_TOKEN=<paste your token>
   ```
3. **Try the CLI:**
   ```bash
   cd cli && cargo run -- search orders
   ```
4. **Try the action locally** with [`act`](https://github.com/nektos/act):
   ```bash
   make demo
   ```
5. **Try the extension:** open this repo in VS Code, run `cd vscode-extension && npm install`, press `F5` to launch the Extension Development Host, then hover a table name in any `.sql` file.

## Architecture

All three branches use the same two env vars (`OPENMETADATA_HOST`, `OPENMETADATA_JWT_TOKEN`) and the same REST endpoints (`/api/v1/search/query`, `/api/v1/tables/{fqn}`, `/api/v1/lineage/table/{id}`, `/api/v1/dataQuality/testSuites`). The CLI also speaks MCP, proxying `{host}/mcp` so you can plug `ometa mcp` into any MCP-aware AI client.

## Repo layout

```
metatree/
├── action/               GitHub Action (Python)
├── cli/                  Rust CLI (`ometa`)
├── vscode-extension/     VS Code extension (TypeScript)
├── .github/workflows/    demo workflow exercising the action
├── docker-compose.yml    local OpenMetadata stack
├── Makefile              install-all / test-all / demo / lint
└── .env.example
```

## Development

```bash
make install-all   # python + rust + node dependencies
make test-all      # full test suite (mocked, no network needed)
make lint          # ruff + clippy + eslint
```

---

*Built for the [OpenMetadata WeMakeDevs Hackathon 2026](https://wemakedevs.org) · Developer Tooling & CI/CD track.*
