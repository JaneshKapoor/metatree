"""MetaTree GitHub Action: downstream-impact analysis for schema/SQL/dbt changes.

Reads a list of changed file paths, extracts entity identifiers, resolves them
against an OpenMetadata catalog, walks downstream lineage, and emits a Markdown
report. Optionally posts the report as a PR comment and/or fails the workflow
when impact is detected.

All HTTP calls handle 401/404/429/5xx explicitly so partial catalog gaps never
crash the action.
"""

from __future__ import annotations

import json
import logging
import os
import re
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable

import requests
import yaml

LOG = logging.getLogger("metatree")
logging.basicConfig(level=logging.INFO, format="[metatree] %(message)s")

SQL_KEYWORDS = {
    "select", "from", "where", "join", "on", "and", "or", "not", "in", "is",
    "null", "true", "false", "as", "with", "by", "group", "order", "having",
    "limit", "offset", "union", "all", "distinct", "case", "when", "then",
    "else", "end", "left", "right", "inner", "outer", "cross", "full", "using",
    "create", "table", "alter", "drop", "view", "if", "exists", "values",
    "insert", "update", "delete", "set", "into",
}

# Heuristic regexes for SQL: capture the qualified or unqualified identifier
# that follows FROM / JOIN / CREATE TABLE / ALTER TABLE / INSERT INTO / UPDATE.
SQL_REFERENCE_RE = re.compile(
    r"\b(?:from|join|create\s+(?:or\s+replace\s+)?table(?:\s+if\s+not\s+exists)?|"
    r"alter\s+table|insert\s+into|update)\s+([A-Za-z_][\w]*(?:\.[A-Za-z_][\w]*){0,2})",
    re.IGNORECASE,
)


@dataclass
class Config:
    host: str
    token: str
    changed_files: list[str]
    fail_on_impact: bool
    comment_on_pr: bool
    github_token: str | None
    github_repo: str | None
    github_event_path: str | None
    step_summary_path: str | None

    @classmethod
    def from_env(cls, env: dict[str, str] | None = None) -> "Config":
        env = env if env is not None else os.environ
        host = (env.get("OPENMETADATA_HOST") or "").rstrip("/")
        token = env.get("OPENMETADATA_JWT_TOKEN") or ""
        if not host or not token:
            raise SystemExit(
                "ACTION REQUIRED: Set OPENMETADATA_HOST and OPENMETADATA_JWT_TOKEN "
                "in your .env file. See .env.example for instructions on where to "
                "find these values."
            )
        raw_files = env.get("METATREE_CHANGED_FILES") or ""
        files = [p for p in re.split(r"[\s\n]+", raw_files.strip()) if p]
        return cls(
            host=host,
            token=token,
            changed_files=files,
            fail_on_impact=_bool(env.get("METATREE_FAIL_ON_IMPACT")),
            comment_on_pr=_bool(env.get("METATREE_COMMENT_ON_PR"), default=True),
            github_token=env.get("GITHUB_TOKEN"),
            github_repo=env.get("GITHUB_REPOSITORY"),
            github_event_path=env.get("GITHUB_EVENT_PATH"),
            step_summary_path=env.get("GITHUB_STEP_SUMMARY"),
        )


def _bool(value: str | None, default: bool = False) -> bool:
    if value is None or value == "":
        return default
    return value.strip().lower() in {"1", "true", "yes", "y", "on"}


@dataclass
class ImpactedAsset:
    name: str
    fqn: str
    kind: str
    owner: str
    url: str


@dataclass
class EntityImpact:
    source_file: str
    extracted_name: str
    matched_fqn: str | None = None
    matched_kind: str | None = None
    downstream: list[ImpactedAsset] = field(default_factory=list)
    not_found: bool = False


# ---------------------------------------------------------------------------
# File parsing
# ---------------------------------------------------------------------------


def extract_entities(file_path: str, content: str) -> list[str]:
    """Extract candidate entity names from a single file based on its extension."""
    suffix = Path(file_path).suffix.lower()
    if suffix == ".sql":
        return _extract_sql_entities(content)
    if suffix in {".yml", ".yaml"}:
        return _extract_dbt_entities(content)
    if suffix == ".json":
        return _extract_schema_entities(content)
    LOG.info("skipping unsupported file: %s", file_path)
    return []


def _extract_sql_entities(content: str) -> list[str]:
    names: list[str] = []
    seen: set[str] = set()
    for match in SQL_REFERENCE_RE.finditer(content):
        ref = match.group(1)
        last = ref.split(".")[-1].lower()
        if last in SQL_KEYWORDS:
            continue
        if ref.lower() in seen:
            continue
        seen.add(ref.lower())
        names.append(ref)
    return names


def _extract_dbt_entities(content: str) -> list[str]:
    try:
        loaded = yaml.safe_load(content)
    except yaml.YAMLError:
        return []
    if not isinstance(loaded, dict):
        return []
    models = loaded.get("models")
    if not isinstance(models, list):
        return []
    names: list[str] = []
    for entry in models:
        if isinstance(entry, dict):
            name = entry.get("name")
            if isinstance(name, str) and name:
                names.append(name)
    return names


def _extract_schema_entities(content: str) -> list[str]:
    try:
        loaded = json.loads(content)
    except json.JSONDecodeError:
        return []
    if not isinstance(loaded, dict):
        return []
    schema = loaded.get("$schema")
    if not isinstance(schema, str) or "table" not in schema.lower():
        return []
    name = loaded.get("name") or loaded.get("title")
    return [name] if isinstance(name, str) and name else []


# ---------------------------------------------------------------------------
# OpenMetadata client (small, focused, dependency-light)
# ---------------------------------------------------------------------------


class OpenMetadataError(RuntimeError):
    pass


class OpenMetadataClient:
    def __init__(self, host: str, token: str, session: requests.Session | None = None) -> None:
        self.host = host.rstrip("/")
        self.token = token
        self.session = session or requests.Session()

    def _request(self, method: str, path: str, **kwargs: Any) -> Any:
        url = f"{self.host}{path}"
        headers = {
            "Authorization": f"Bearer {self.token}",
            "Accept": "application/json",
        }
        for attempt in range(3):
            response = self.session.request(method, url, headers=headers, timeout=30, **kwargs)
            if response.status_code == 401:
                raise OpenMetadataError(
                    f"401 Unauthorized for {url}. Check OPENMETADATA_JWT_TOKEN."
                )
            if response.status_code == 404:
                return None
            if response.status_code == 429:
                wait = float(response.headers.get("Retry-After") or 2 ** attempt)
                LOG.warning("rate-limited by %s, sleeping %.1fs", url, wait)
                time.sleep(wait)
                continue
            if 500 <= response.status_code < 600:
                if attempt < 2:
                    time.sleep(2 ** attempt)
                    continue
                raise OpenMetadataError(
                    f"{response.status_code} from {url}. Check OPENMETADATA_HOST."
                )
            if not response.ok:
                raise OpenMetadataError(f"{response.status_code} from {url}: {response.text[:200]}")
            if not response.content:
                return None
            return response.json()
        raise OpenMetadataError(f"Exceeded retries calling {url}")

    def search(self, query: str, index: str = "table_search_index", limit: int = 5) -> list[dict[str, Any]]:
        result = self._request("GET", "/v1/search/query", params={
            "q": query, "index": index, "limit": limit,
        })
        if not result:
            return []
        hits = result.get("hits", {}).get("hits", [])
        return hits if isinstance(hits, list) else []

    def lineage(self, entity_id: str, upstream: int = 0, downstream: int = 3) -> dict[str, Any] | None:
        return self._request("GET", f"/v1/lineage/table/{entity_id}", params={
            "upstreamDepth": upstream, "downstreamDepth": downstream,
        })


# ---------------------------------------------------------------------------
# Impact analysis core
# ---------------------------------------------------------------------------


def analyze(cfg: Config, client: OpenMetadataClient, read_file=Path) -> list[EntityImpact]:
    impacts: list[EntityImpact] = []
    for file_path in cfg.changed_files:
        try:
            content = read_file(file_path).read_text(encoding="utf-8", errors="replace")
        except FileNotFoundError:
            LOG.warning("changed file not found on disk: %s", file_path)
            continue
        for name in extract_entities(file_path, content):
            impacts.append(_analyze_single(client, file_path, name))
    return impacts


def _analyze_single(client: OpenMetadataClient, source_file: str, name: str) -> EntityImpact:
    impact = EntityImpact(source_file=source_file, extracted_name=name)
    short = name.split(".")[-1]
    hits = client.search(query=short, index="table_search_index", limit=5)
    best = _best_hit(hits, short)
    if best is None:
        impact.not_found = True
        return impact
    impact.matched_fqn = best.get("fullyQualifiedName") or best.get("name") or short
    impact.matched_kind = best.get("entityType") or "table"
    entity_id = best.get("id")
    if not entity_id:
        return impact
    lineage = client.lineage(entity_id, upstream=0, downstream=3)
    if lineage:
        impact.downstream = _downstream_assets(lineage)
    return impact


def _best_hit(hits: list[dict[str, Any]], query: str) -> dict[str, Any] | None:
    if not hits:
        return None
    top = hits[0]
    score = float(top.get("_score") or 0)
    if score <= 0.5:
        return None
    src = top.get("_source") or {}
    src.setdefault("id", top.get("_id"))
    return src


def _downstream_assets(lineage: dict[str, Any]) -> list[ImpactedAsset]:
    nodes_by_id: dict[str, dict[str, Any]] = {}
    for node in lineage.get("nodes") or []:
        if isinstance(node, dict) and node.get("id"):
            nodes_by_id[node["id"]] = node
    downstream_ids: list[str] = []
    seen: set[str] = set()
    for edge in lineage.get("downstreamEdges") or []:
        if not isinstance(edge, dict):
            continue
        target = edge.get("toEntity")
        if isinstance(target, dict):
            to_id = target.get("id")
        elif isinstance(target, str):
            to_id = target
        else:
            to_id = None
        if isinstance(to_id, str) and to_id not in seen:
            seen.add(to_id)
            downstream_ids.append(to_id)
    assets: list[ImpactedAsset] = []
    for node_id in downstream_ids:
        node = nodes_by_id.get(node_id, {"id": node_id})
        assets.append(_node_to_asset(node))
    return assets


def _node_to_asset(node: dict[str, Any]) -> ImpactedAsset:
    name = node.get("name") or node.get("displayName") or node.get("id") or "unknown"
    fqn = node.get("fullyQualifiedName") or name
    kind = node.get("type") or node.get("entityType") or "table"
    owner = "—"
    owners = node.get("owners") or node.get("owner")
    if isinstance(owners, list) and owners:
        first = owners[0]
        if isinstance(first, dict):
            owner = first.get("displayName") or first.get("name") or owner
    elif isinstance(owners, dict):
        owner = owners.get("displayName") or owners.get("name") or owner
    href = node.get("href") or ""
    return ImpactedAsset(name=str(name), fqn=str(fqn), kind=str(kind).title(), owner=str(owner), url=str(href))


# ---------------------------------------------------------------------------
# Markdown report
# ---------------------------------------------------------------------------


def render_report(cfg: Config, impacts: list[EntityImpact]) -> str:
    changed = ", ".join(f"`{f}`" for f in cfg.changed_files) or "_(none)_"
    found_count = sum(1 for i in impacts if i.matched_fqn)
    rows: list[str] = []
    not_found: list[str] = []
    for impact in impacts:
        if impact.not_found:
            not_found.append(impact.extracted_name)
            continue
        if not impact.matched_fqn:
            continue
        if not impact.downstream:
            rows.append(
                f"| `{impact.extracted_name}` | _no downstream assets_ | — | — | — |"
            )
            continue
        for asset in impact.downstream:
            link = f"[View]({asset.url})" if asset.url else "—"
            rows.append(
                f"| `{impact.extracted_name}` | `{asset.name}` | {asset.kind} | {asset.owner} | {link} |"
            )

    parts: list[str] = [
        "## 🌳 MetaTree Impact Analysis",
        "",
        f"**Triggered by:** {changed}",
        f"**Analyzed:** {found_count} entit{'y' if found_count == 1 else 'ies'} in OpenMetadata",
        "",
        "### ⚠️ Downstream Impact",
        "",
        "| Changed Entity | Downstream Asset | Type | Owner | Link |",
        "|---|---|---|---|---|",
    ]
    if rows:
        parts.extend(rows)
    else:
        parts.append("| _none_ | — | — | — | — |")
    parts.append("")
    parts.append("### ✅ No Impact Found")
    if not_found:
        for name in not_found:
            parts.append(f"- `{name}` — not found in OpenMetadata catalog")
    else:
        parts.append("- _all changed entities had downstream lineage resolved_")
    parts.extend([
        "",
        "---",
        "*Generated by [MetaTree](https://github.com/JaneshKapoor/metatree) · "
        "[OpenMetadata](https://open-metadata.org)*",
        "",
    ])
    return "\n".join(parts)


def has_downstream_impact(impacts: list[EntityImpact]) -> bool:
    return any(i.downstream for i in impacts)


# ---------------------------------------------------------------------------
# GitHub PR comment
# ---------------------------------------------------------------------------


def post_pr_comment(cfg: Config, body: str) -> bool:
    if not cfg.comment_on_pr:
        return False
    if not (cfg.github_token and cfg.github_repo and cfg.github_event_path):
        LOG.info("skipping PR comment: missing GITHUB_TOKEN/REPOSITORY/EVENT_PATH")
        return False
    try:
        event = json.loads(Path(cfg.github_event_path).read_text(encoding="utf-8"))
    except (FileNotFoundError, json.JSONDecodeError) as exc:
        LOG.warning("could not read GitHub event payload: %s", exc)
        return False
    pr_number = (event.get("pull_request") or {}).get("number") or event.get("number")
    if not pr_number:
        LOG.info("skipping PR comment: not a pull_request event")
        return False
    url = f"https://api.github.com/repos/{cfg.github_repo}/issues/{pr_number}/comments"
    response = requests.post(
        url,
        headers={
            "Authorization": f"Bearer {cfg.github_token}",
            "Accept": "application/vnd.github+json",
        },
        json={"body": body},
        timeout=30,
    )
    if not response.ok:
        LOG.warning("PR comment failed: %s %s", response.status_code, response.text[:200])
        return False
    return True


def write_step_summary(cfg: Config, body: str) -> None:
    if not cfg.step_summary_path:
        return
    try:
        with open(cfg.step_summary_path, "a", encoding="utf-8") as handle:
            handle.write(body)
            handle.write("\n")
    except OSError as exc:
        LOG.warning("could not write GITHUB_STEP_SUMMARY: %s", exc)


def emit_outputs(impact_found: bool, report_path: str) -> None:
    output_path = os.environ.get("GITHUB_OUTPUT")
    if not output_path:
        return
    try:
        with open(output_path, "a", encoding="utf-8") as handle:
            handle.write(f"impact-found={'true' if impact_found else 'false'}\n")
            handle.write(f"report-path={report_path}\n")
    except OSError as exc:
        LOG.warning("could not write GITHUB_OUTPUT: %s", exc)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main(argv: Iterable[str] | None = None) -> int:
    cfg = Config.from_env()
    if not cfg.changed_files:
        LOG.info("no changed files supplied; nothing to analyze")
        return 0
    client = OpenMetadataClient(cfg.host, cfg.token)
    try:
        impacts = analyze(cfg, client)
    except OpenMetadataError as exc:
        LOG.error(str(exc))
        return 2

    report = render_report(cfg, impacts)
    report_path = os.environ.get("METATREE_REPORT_PATH", "metatree-report.md")
    Path(report_path).write_text(report, encoding="utf-8")
    write_step_summary(cfg, report)
    post_pr_comment(cfg, report)
    impact_found = has_downstream_impact(impacts)
    emit_outputs(impact_found, report_path)
    if cfg.fail_on_impact and impact_found:
        LOG.error("downstream impact detected and fail-on-impact=true")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
