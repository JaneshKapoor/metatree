# MetaTree Action — *"What breaks if I change this?"*

A GitHub Action that runs an OpenMetadata-powered impact analysis on every PR, then posts a Markdown comment listing the dashboards, pipelines, and tables downstream of whatever you just changed.

## Usage

```yaml
name: PR impact analysis
on: [pull_request]

permissions:
  pull-requests: write
  contents: read

jobs:
  metatree:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Get changed files
        id: changed-files
        uses: tj-actions/changed-files@v45

      - uses: JaneshKapoor/metatree/action@v1
        with:
          openmetadata-host:  ${{ secrets.OPENMETADATA_HOST }}
          openmetadata-token: ${{ secrets.OPENMETADATA_JWT_TOKEN }}
          changed-files:      ${{ steps.changed-files.outputs.all_changed_files }}
          fail-on-impact:     false
          comment-on-pr:      true
```

## Inputs

| Name                  | Required | Default              | Description                                                |
| --------------------- | -------- | -------------------- | ---------------------------------------------------------- |
| `openmetadata-host`   | yes      | —                    | OpenMetadata API base URL (e.g. `…/api`)                   |
| `openmetadata-token`  | yes      | —                    | JWT token; copy from Settings → Bots → `ingestion-bot`     |
| `changed-files`       | yes      | —                    | Space- or newline-separated list of paths                  |
| `fail-on-impact`      | no       | `false`              | Fail the workflow if any downstream impact is detected     |
| `comment-on-pr`       | no       | `true`               | Post the report as a PR comment                            |
| `github-token`        | no       | `${{ github.token }}`| Used to post the PR comment                                |

## Outputs

| Name           | Description                                            |
| -------------- | ------------------------------------------------------ |
| `impact-found` | `true` if any downstream asset was identified          |
| `report-path`  | Path to the generated Markdown report on the runner    |

## How it parses files

| Extension       | Heuristic                                                                    |
| --------------- | ---------------------------------------------------------------------------- |
| `.sql`          | regex over `FROM`, `JOIN`, `CREATE TABLE`, `ALTER TABLE`, `INSERT INTO`      |
| `.yml` / `.yaml`| if `models:` key present, treat as dbt and extract each `name:` field        |
| `.json`         | if `$schema` mentions `table`, treat as schema and extract `name` / `title`  |
| anything else   | skipped with an info log                                                     |

Each extracted name is searched against `table_search_index`; the top hit is kept when its score exceeds 0.5. The matched entity's downstream lineage is fetched up to 3 hops, and every downstream asset is added to the report.

## Local development & tests

```bash
pip install -r requirements.txt
pytest -q
```

The tests use `responses` to mock the OpenMetadata API, so no live host or JWT is required.

## Error handling

| HTTP | Behavior                                                                              |
| ---- | ------------------------------------------------------------------------------------- |
| 401  | Action exits 2 with a clear "check OPENMETADATA_JWT_TOKEN" message                    |
| 404  | Treated as "entity not found"; appears in the *No Impact Found* section of the report |
| 429  | Honors `Retry-After` (or exponential backoff) and retries up to 3 times               |
| 5xx  | Retries with backoff; final failure exits 2 with a "check OPENMETADATA_HOST" message  |
