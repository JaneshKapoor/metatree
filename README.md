# MetaTree

> **OpenMetadata lives where developers work — in your terminal, your CI, and your editor.**

![Python](https://img.shields.io/badge/Python-3.11%2B-3776AB?logo=python&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=white)
![TypeScript](https://img.shields.io/badge/TypeScript-strict-3178C6?logo=typescript&logoColor=white)
![tests](https://img.shields.io/badge/tests-32_passing-2EA043)

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
- uses: JaneshKapoor/metatree/action@v1
  with:
    openmetadata-host:  ${{ secrets.OPENMETADATA_HOST }}
    openmetadata-token: ${{ secrets.OPENMETADATA_JWT_TOKEN }}
    changed-files:      ${{ steps.changed-files.outputs.all_changed_files }}
    fail-on-impact:     false
    comment-on-pr:      true
```

See [`action/README.md`](action/README.md). Tests: 15 pytest cases, all using `responses` for HTTP mocking.

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

See [`cli/README.md`](cli/README.md). Tests: 9 cargo unit/integration cases, mocked with `mockito`. Compiles on stable Rust with `#![deny(warnings)]`.

## Branch 3 — VS Code Extension

Surface OpenMetadata where you write SQL and dbt. Hover any table or column to see owner, tags, description, and a column list; press `Ctrl+Shift+M` to open an interactive lineage graph; the sidebar shows DQ status and failing tests for everything you've touched recently.

```bash
cd vscode-extension
npm install
npm run package          # produces metatree-vscode.vsix
code --install-extension metatree-vscode.vsix
```

See [`vscode-extension/README.md`](vscode-extension/README.md). Tests: 8 pure-function unit cases in plain Node (TypeScript strict mode).

---

## Getting started

You have two options for the OpenMetadata backend. Pick one, paste the JWT into `.env`, and every branch will pick it up automatically.

### Option A — public sandbox (zero install)

1. Visit <https://sandbox.open-metadata.org> and sign in.
2. Bottom-left gear → **Settings** → **Bots** → click `ingestion-bot` → copy **JWT Token**.
3. Copy `.env.example` to `.env` and fill in:
   ```bash
   OPENMETADATA_HOST=https://sandbox.open-metadata.org/api
   OPENMETADATA_JWT_TOKEN=<paste your sandbox token>
   ```

### Option B — local stack (full control, no rate limits)

Bring up the bundled OpenMetadata 1.5.0 stack (MySQL + Elasticsearch + migrations + server) and pull the JWT programmatically — no UI clicking required.

```bash
docker compose up -d                          # ~3-5 min on first run (~3 GB pull)
python scripts/get_local_jwt.py --write-env   # logs in as admin, fetches the bot JWT, writes .env
```

The helper script:

1. Polls `http://localhost:8585/api/v1/system/version` until the server is reachable.
2. POSTs `/api/v1/users/login` with the default `admin@open-metadata.org / admin` credentials (and signs the admin up first if it's a brand-new install).
3. GETs `/api/v1/users/auth-mechanism/{ingestion-bot.id}` and prints + persists the JWT.

After it returns, your `.env` will have `OPENMETADATA_HOST=http://localhost:8585/api` and `OPENMETADATA_JWT_TOKEN=<extracted token>`. Both keys are gitignored.

If you'd rather click through the UI, the path is the same as the sandbox: open `http://localhost:8585`, login as `admin@open-metadata.org / admin`, then **Settings → Bots → ingestion-bot → JWT Token**.

### Try each branch

```bash
# CLI
cd cli && cargo run -- search orders

# Action (locally via `act`)
make demo

# VS Code extension
cd vscode-extension && npm install
# then press F5 in VS Code to launch the Extension Development Host,
# open any .sql file, and hover over a table name.
```

## Architecture

All three branches read the same two env vars (`OPENMETADATA_HOST`, `OPENMETADATA_JWT_TOKEN`) and the same REST endpoints:

- `GET /api/v1/search/query?q=…&index=…_search_index&limit=N`
- `GET /api/v1/tables/name/{fqn}?fields=owners,tags,columns,…`
- `GET /api/v1/lineage/table/{id}?upstreamDepth=N&downstreamDepth=N`
- `GET /api/v1/dataQuality/testSuites?entityLink=<#E::table::FQN>&fields=tests,testCaseResults`
- `PATCH /api/v1/tables/name/{fqn}` (JSON-Patch / RFC 6902)

The CLI also exposes `ometa mcp --port 3000`, a thin proxy that forwards every request to `{host}/mcp` with the JWT injected, so any MCP-aware client (Claude Desktop, Cursor) can use OpenMetadata without each one juggling tokens.

Every HTTP path handles `401`, `404`, `429`, and `5xx` explicitly — no silent failures.

## Repo layout

```
metatree/
├── action/                  GitHub Action (Python 3.11)
│   ├── action.yml           composite action definition
│   ├── src/impact_analysis.py
│   └── tests/               pytest + responses, 15 cases
├── cli/                     Rust CLI `ometa`
│   ├── Cargo.toml
│   ├── src/{client,config,spec,main}.rs
│   ├── src/commands/{configure,search,describe,lineage,quality,patch,mcp}.rs
│   └── tests/integration.rs cargo test, 9 cases
├── vscode-extension/        TypeScript extension (strict mode)
│   ├── package.json
│   ├── src/{extension,client,parsing,quality,lineageHtml}.ts
│   ├── src/providers/{hoverProvider,lineageProvider,qualityProvider}.ts
│   └── src/test/runUnit.ts  Node unit suite, 8 cases
├── scripts/get_local_jwt.py admin login + bot JWT extractor
├── .github/workflows/demo.yml
├── docker-compose.yml       MySQL + Elasticsearch + migrate + OM 1.5.0
├── Makefile                 install-all / test-all / demo / lint / build-cli / build-extension
├── .env.example
└── README.md
```

## Test status (last run)

| Suite | Result | Mocked? |
|---|---|---|
| `cd action && pytest -q` | **15 passed in 0.36s** | yes (`responses`) |
| `cd cli && cargo test` | **9 passed** (8 unit + 1 integration) | yes (`mockito`) |
| `cd vscode-extension && npm test` | **8 passed** | yes (pure Node) |
| `cargo build` | clean (`#![deny(warnings)]` enforced) | n/a |
| `npx vsce package` | `metatree-vscode.vsix` (40.64 KB, 35 files) | n/a |

### Live verification against the local stack

After `make local-up && make local-jwt && python scripts/seed_local_sample.py`:

| Live test | Result |
|---|---|
| `ometa search orders` | returns 2 hits: `orders` (score 52.7) and `daily_revenue` (14.5) |
| `ometa describe sample_mysql.demo_db.public.orders` | renders 5-column table (id/customer_id/total_amount/status/created_at) with description |
| `ometa lineage sample_mysql.demo_db.public.orders --depth 2` | ASCII tree shows `downstream → daily_revenue` |
| `ometa quality sample_mysql.demo_db.public.orders` | "No data-quality tests found" (no DQ tests seeded) |
| Action's `impact_analysis.py` on a SQL file with `FROM orders JOIN customers …` | report identifies `orders → daily_revenue` and `customers → daily_revenue` correctly |

## Development

```bash
make install-all   # python + rust + node dependencies
make test-all      # full test suite (mocked, no network needed)
make lint          # ruff + clippy + eslint
```

---

*Built for the [OpenMetadata WeMakeDevs Hackathon 2026](https://wemakedevs.org) · Developer Tooling & CI/CD track.*
