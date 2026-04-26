# MetaTree — 2-minute YouTube script

Total runtime: ~1:55. Spoken word count: ~280 (≈140 wpm, conversational).
Three voiceover columns: **time**, **on-screen visual**, **what you say**.

> **Recording tips**
> - Use OBS or Loom; 1920×1080, 30 fps, system audio off, mic on.
> - Pre-stage two terminals (one for the CLI, one for the action) and a VS Code window with `demo/orders_change.sql` open so you can hover.
> - Hide the taskbar / personal tabs / `.env` files before you record.
> - The whole demo script is reproducible — every line below comes from `make local-up && make local-jwt && python scripts/seed_local_sample.py`.

---

## 0:00 – 0:08 · Hook  *(8 seconds)*

**Visual:** terminal showing a `git diff` on a `.sql` file — a column being renamed in a table called `orders`.

**Voiceover:**
> "You just renamed a column in `orders`. Are you sure no dashboard, pipeline, or downstream model just broke?"

**On-screen text:** *"What breaks when you change this?"*

---

## 0:08 – 0:25 · Pitch  *(17 seconds)*

**Visual:** the README header from `github.com/JaneshKapoor/metatree`. The ASCII architecture diagram comes into focus.

**Voiceover:**
> "I built MetaTree. It's three tools, one OpenMetadata catalog. A GitHub Action that comments on PRs, a Rust CLI for your terminal, and a VS Code extension for your editor — all sharing the same backend."

**On-screen text:**
- *MetaTree*
- *Action · CLI · Extension*
- *github.com/JaneshKapoor/metatree*

---

## 0:25 – 0:55 · Demo 1, the GitHub Action  *(30 seconds)*

**Visual:** split-screen — left: a sample PR view; right: `metatree-report.md` rendered in VS Code's markdown preview.

**Voiceover:**
> "When a PR touches a SQL or dbt file, MetaTree's action parses it, hits OpenMetadata, walks downstream lineage, and posts this comment. Here, changing `orders` and `customers` would impact the `daily_revenue` table — surfaced before the merge, not after the dashboard breaks."

**On-screen text:**
- *🌳 MetaTree Impact Analysis*
- *Downstream: daily_revenue · Owner: data-team*

---

## 0:55 – 1:22 · Demo 2, the CLI  *(27 seconds)*

**Visual:** a clean terminal. Type each command live (or pre-record the typing and play it at 1.0×).

```bash
ometa search orders
ometa describe sample_mysql.demo_db.public.orders
ometa lineage  sample_mysql.demo_db.public.orders --depth 2
```

**Voiceover:**
> "Same backend, different surface. `ometa search` finds anything in your catalog by name. `ometa describe` prints the column table, owners, tags. `ometa lineage` shows you upstream and downstream as an ASCII tree — perfect for terminals, perfect for AI agents. There's also `ometa mcp`, which exposes the catalog as an MCP server for Claude or Cursor."

**On-screen text:** *Rust · cross-platform · `cargo install --path cli`*

---

## 1:22 – 1:48 · Demo 3, the VS Code extension  *(26 seconds)*

**Visual:** VS Code with a `.sql` file open. Hover over `orders`. Tooltip pops up with owner, tags, columns. Press `Ctrl+Shift+M`. The lineage webview slides in from the right with the `vis-network` graph — `orders` highlighted, `daily_revenue` on the right.

**Voiceover:**
> "And right where you write SQL — hover any table for owner, tags, columns. Press Ctrl-Shift-M to pop open an interactive lineage graph. The sidebar tracks data quality across everything you've touched today."

**On-screen text:** *VS Code · TypeScript strict · 40 KB extension*

---

## 1:48 – 1:55 · Outro  *(7 seconds)*

**Visual:** GitHub repo page with the v0.1.0 release pinned.

**Voiceover:**
> "MetaTree. One repo, three branches. Built for the WeMakeDevs OpenMetadata Hackathon. Link's below — try it in five minutes."

**On-screen text:**
- *github.com/JaneshKapoor/metatree*
- *#OpenMetadata · #WeMakeDevs · #Hackathon2026*

---

## YouTube description (paste this under the video)

> MetaTree brings OpenMetadata to every part of a developer's day.
>
> 🌳 **GitHub Action** — automatic downstream-impact comments on every PR.
> 🦀 **`ometa` CLI** (Rust) — search, describe, lineage, MCP proxy.
> 🧩 **VS Code extension** — hover tooltips, interactive lineage, DQ sidebar.
>
> One project, three branches, one OpenMetadata backend.
>
> Repo: https://github.com/JaneshKapoor/metatree
> Release: https://github.com/JaneshKapoor/metatree/releases/tag/v0.1.0
>
> Built for the WeMakeDevs OpenMetadata Hackathon 2026 — Developer Tooling & CI/CD track.
>
> Chapters:
> 00:00 What breaks when you change this?
> 00:08 The pitch
> 00:25 GitHub Action — PR impact comments
> 00:55 `ometa` CLI demo
> 01:22 VS Code extension demo
> 01:48 Wrap

## Quick screen-recording checklist

- [ ] `make local-up && make local-jwt && python scripts/seed_local_sample.py` finished, `.env` populated
- [ ] `ometa --help` reachable in your shell (`export PATH="$HOME/.cargo/bin:$PATH"`)
- [ ] VS Code: extension installed, `metatree.host` and `metatree.token` set
- [ ] Sample SQL file open (`demo/orders_change.sql` containing `SELECT * FROM orders JOIN customers ...`)
- [ ] Display set to 1920×1080, dark theme, font size +2 for readability
- [ ] Hide bookmarks bar, status bar clutter, system notifications
- [ ] Mic check: -12 dB peak, no background fan/AC
