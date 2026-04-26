# MetaTree — OpenMetadata for Developers (VS Code)

Surface OpenMetadata where you actually write SQL and dbt models. No more tab-switching to look up an owner or guess at lineage.

## Features

- **Hover any table or column** in `.sql` or dbt `.yml` files to see owner, tags, description, and a column list pulled live from your OpenMetadata catalog.
- **Press `Ctrl+Shift+M`** (`Cmd+Shift+M` on Mac) on a table reference to open an interactive lineage graph powered by vis-network — color-coded by entity type, with the current node highlighted.
- **Sidebar** showing recently-viewed tables with their DQ score, plus a list of currently failing tests across everything you've touched.
- Hover results are **cached for 5 minutes** per word so the API doesn't take a beating while you scroll.

## Configure

Open VS Code settings (`Ctrl+,`) and search for "metatree":

| Setting              | Description                                                      |
| -------------------- | ---------------------------------------------------------------- |
| `metatree.host`      | OpenMetadata API base URL, e.g. `https://sandbox.open-metadata.org/api` |
| `metatree.token`     | JWT token from Settings → Bots → ingestion-bot                  |
| `metatree.enabled`   | Master switch                                                    |

Configuration changes take effect immediately — no reload required.

## Develop

```bash
npm install
npm run compile
npm test    # runs the unit suite (no VS Code host required)
```

To debug in VS Code: open this folder, press `F5` to launch an Extension Development Host, then open any `.sql` file and hover over a table name.

## Package a `.vsix`

```bash
npm install
npx @vscode/vsce package --out metatree-vscode.vsix
```

## Error handling

| HTTP | Behavior                                                                |
| ---- | ----------------------------------------------------------------------- |
| 401  | Toast asks the user to re-check `metatree.token`                        |
| 404  | Hover shows nothing (we treat the word as not in catalog)               |
| 429  | Honors `Retry-After`, retries up to 3 times                              |
| 5xx  | Retries up to 3 times, then surfaces the error in the developer console |
