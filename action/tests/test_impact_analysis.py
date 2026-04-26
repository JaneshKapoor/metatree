"""Unit tests for the MetaTree GitHub Action.

We mock OpenMetadata via the `responses` library so tests never touch the network.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest
import responses

from src import impact_analysis as ia


HOST = "https://example.test/api"


# ---------------------------------------------------------------------------
# Entity extraction
# ---------------------------------------------------------------------------


def test_sql_extraction_picks_up_from_join_create_alter():
    sql = """
    SELECT *
    FROM orders
    JOIN customers ON orders.customer_id = customers.id;

    CREATE TABLE IF NOT EXISTS daily_revenue AS SELECT 1;
    ALTER TABLE staging_temp ADD COLUMN x INT;
    INSERT INTO archive.events VALUES (1);
    """
    names = ia.extract_entities("models/foo.sql", sql)
    assert "orders" in names
    assert "customers" in names
    assert "daily_revenue" in names
    assert "staging_temp" in names
    assert "archive.events" in names


def test_sql_extraction_skips_keywords():
    sql = "SELECT * FROM SELECT JOIN where"
    names = ia.extract_entities("foo.sql", sql)
    assert all(n.lower() not in ia.SQL_KEYWORDS for n in names)


def test_dbt_yaml_extraction_returns_model_names():
    yaml_doc = """
    version: 2
    models:
      - name: dim_customers
        description: customer dimension
      - name: fct_orders
    """
    names = ia.extract_entities("models/schema.yml", yaml_doc)
    assert names == ["dim_customers", "fct_orders"]


def test_dbt_yaml_without_models_key_is_skipped():
    yaml_doc = """version: 2\nseeds:\n  - name: foo"""
    assert ia.extract_entities("seeds.yml", yaml_doc) == []


def test_json_schema_extraction():
    payload = json.dumps({
        "$schema": "https://example.com/schemas/table-schema.json",
        "name": "user_events",
    })
    assert ia.extract_entities("schemas/user_events.json", payload) == ["user_events"]


def test_unknown_extension_is_skipped(caplog):
    assert ia.extract_entities("README.md", "# hello") == []


# ---------------------------------------------------------------------------
# OpenMetadataClient HTTP behavior
# ---------------------------------------------------------------------------


@responses.activate
def test_search_returns_hits():
    responses.get(
        f"{HOST}/v1/search/query",
        json={"hits": {"hits": [{"_id": "abc", "_score": 1.5, "_source": {"name": "orders"}}]}},
        status=200,
    )
    client = ia.OpenMetadataClient(HOST, "tok")
    hits = client.search("orders")
    assert hits and hits[0]["_id"] == "abc"


@responses.activate
def test_search_404_returns_empty_list():
    responses.get(f"{HOST}/v1/search/query", status=404)
    client = ia.OpenMetadataClient(HOST, "tok")
    assert client.search("nothing") == []


@responses.activate
def test_401_raises_clear_error():
    responses.get(f"{HOST}/v1/search/query", status=401)
    client = ia.OpenMetadataClient(HOST, "bad")
    with pytest.raises(ia.OpenMetadataError, match="OPENMETADATA_JWT_TOKEN"):
        client.search("foo")


@responses.activate
def test_429_then_200_is_retried():
    responses.get(f"{HOST}/v1/search/query", status=429, headers={"Retry-After": "0"})
    responses.get(
        f"{HOST}/v1/search/query",
        json={"hits": {"hits": []}},
        status=200,
    )
    client = ia.OpenMetadataClient(HOST, "tok")
    assert client.search("foo") == []


# ---------------------------------------------------------------------------
# End-to-end analyze + report
# ---------------------------------------------------------------------------


def _setup_search_and_lineage(name: str, table_id: str = "tid-1"):
    responses.get(
        f"{HOST}/v1/search/query",
        json={
            "hits": {"hits": [{
                "_id": table_id,
                "_score": 2.5,
                "_source": {
                    "id": table_id,
                    "name": name,
                    "fullyQualifiedName": f"db.public.{name}",
                    "entityType": "table",
                },
            }]},
        },
        status=200,
    )
    responses.get(
        f"{HOST}/v1/lineage/table/{table_id}",
        json={
            "nodes": [
                {"id": "n1", "name": "revenue_dashboard", "type": "dashboard",
                 "fullyQualifiedName": "bi.revenue_dashboard",
                 "owners": [{"displayName": "data-team"}],
                 "href": "https://example.test/dashboard/n1"},
                {"id": "n2", "name": "daily_orders_pipeline", "type": "pipeline",
                 "fullyQualifiedName": "etl.daily_orders_pipeline",
                 "owners": [{"displayName": "eng-team"}]},
            ],
            "downstreamEdges": [
                {"fromEntity": table_id, "toEntity": "n1"},
                {"fromEntity": table_id, "toEntity": "n2"},
            ],
        },
        status=200,
    )


@responses.activate
def test_analyze_end_to_end_builds_report(tmp_path: Path):
    sql_file = tmp_path / "models" / "orders.sql"
    sql_file.parent.mkdir(parents=True)
    sql_file.write_text("SELECT * FROM orders WHERE 1=1;")

    _setup_search_and_lineage("orders")

    cfg = ia.Config(
        host=HOST, token="tok",
        changed_files=[str(sql_file)],
        fail_on_impact=False, comment_on_pr=False,
        github_token=None, github_repo=None,
        github_event_path=None, step_summary_path=None,
    )
    client = ia.OpenMetadataClient(HOST, "tok")
    impacts = ia.analyze(cfg, client)
    assert len(impacts) == 1
    assert impacts[0].matched_fqn == "db.public.orders"
    assert {a.name for a in impacts[0].downstream} == {"revenue_dashboard", "daily_orders_pipeline"}

    report = ia.render_report(cfg, impacts)
    assert "🌳 MetaTree Impact Analysis" in report
    assert "revenue_dashboard" in report
    assert "Dashboard" in report
    assert ia.has_downstream_impact(impacts) is True


@responses.activate
def test_report_lists_not_found_when_no_match(tmp_path: Path):
    sql_file = tmp_path / "x.sql"
    sql_file.write_text("CREATE TABLE staging_temp AS SELECT 1;")
    responses.get(
        f"{HOST}/v1/search/query",
        json={"hits": {"hits": []}},
        status=200,
    )
    cfg = ia.Config(
        host=HOST, token="tok", changed_files=[str(sql_file)],
        fail_on_impact=False, comment_on_pr=False,
        github_token=None, github_repo=None,
        github_event_path=None, step_summary_path=None,
    )
    client = ia.OpenMetadataClient(HOST, "tok")
    impacts = ia.analyze(cfg, client)
    report = ia.render_report(cfg, impacts)
    assert "staging_temp" in report
    assert "No Impact Found" in report
    assert ia.has_downstream_impact(impacts) is False


# ---------------------------------------------------------------------------
# Exit code on fail-on-impact
# ---------------------------------------------------------------------------


@responses.activate
def test_main_exits_1_when_fail_on_impact_and_impact_present(tmp_path: Path, monkeypatch):
    sql_file = tmp_path / "orders.sql"
    sql_file.write_text("SELECT * FROM orders;")
    _setup_search_and_lineage("orders")

    monkeypatch.setenv("OPENMETADATA_HOST", HOST)
    monkeypatch.setenv("OPENMETADATA_JWT_TOKEN", "tok")
    monkeypatch.setenv("METATREE_CHANGED_FILES", str(sql_file))
    monkeypatch.setenv("METATREE_FAIL_ON_IMPACT", "true")
    monkeypatch.setenv("METATREE_COMMENT_ON_PR", "false")
    monkeypatch.setenv("METATREE_REPORT_PATH", str(tmp_path / "report.md"))
    monkeypatch.delenv("GITHUB_OUTPUT", raising=False)
    monkeypatch.delenv("GITHUB_STEP_SUMMARY", raising=False)

    rc = ia.main()
    assert rc == 1
    assert (tmp_path / "report.md").exists()


@responses.activate
def test_main_exits_0_when_no_impact(tmp_path: Path, monkeypatch):
    sql_file = tmp_path / "orders.sql"
    sql_file.write_text("SELECT * FROM unknown_table;")
    responses.get(
        f"{HOST}/v1/search/query",
        json={"hits": {"hits": []}},
        status=200,
    )
    monkeypatch.setenv("OPENMETADATA_HOST", HOST)
    monkeypatch.setenv("OPENMETADATA_JWT_TOKEN", "tok")
    monkeypatch.setenv("METATREE_CHANGED_FILES", str(sql_file))
    monkeypatch.setenv("METATREE_FAIL_ON_IMPACT", "true")
    monkeypatch.setenv("METATREE_COMMENT_ON_PR", "false")
    monkeypatch.setenv("METATREE_REPORT_PATH", str(tmp_path / "report.md"))
    monkeypatch.delenv("GITHUB_OUTPUT", raising=False)
    monkeypatch.delenv("GITHUB_STEP_SUMMARY", raising=False)
    assert ia.main() == 0


def test_config_missing_env_raises():
    with pytest.raises(SystemExit, match="ACTION REQUIRED"):
        ia.Config.from_env({})
