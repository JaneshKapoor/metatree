"""Seed a tiny demo dataset into the local OpenMetadata instance so MetaTree's
CLI / Action / extension have something to find on a fresh install.

Creates:
  - database service:       sample_mysql
  - database:               sample_mysql.demo_db
  - schema:                 sample_mysql.demo_db.public
  - tables:                 orders, customers, daily_revenue
  - lineage:                orders -> daily_revenue
                            customers -> daily_revenue

Run with:
    python scripts/seed_local_sample.py
"""

from __future__ import annotations

import os
import sys
import time
from typing import Any

import requests

HOST = os.environ.get("OPENMETADATA_HOST", "http://localhost:8585/api").rstrip("/")
TOKEN = os.environ.get("OPENMETADATA_JWT_TOKEN", "")

if not TOKEN:
    print("OPENMETADATA_JWT_TOKEN not set in env; source .env first", file=sys.stderr)
    sys.exit(2)

HEADERS = {
    "Authorization": f"Bearer {TOKEN}",
    "Accept": "application/json",
    "Content-Type": "application/json",
}


def put(path: str, body: dict[str, Any]) -> dict[str, Any]:
    response = requests.put(f"{HOST}{path}", headers=HEADERS, json=body, timeout=15)
    if not response.ok:
        print(f"PUT {path} -> {response.status_code}: {response.text[:300]}", file=sys.stderr)
        response.raise_for_status()
    if not response.content:
        return {}
    try:
        return response.json()
    except ValueError:
        return {}


def main() -> int:
    print(f"[seed] target: {HOST}")
    service = put("/v1/services/databaseServices", {
        "name": "sample_mysql",
        "serviceType": "Mysql",
        "connection": {
            "config": {
                "type": "Mysql",
                "username": "demo",
                "authType": {"password": "demo"},
                "hostPort": "localhost:3306",
            },
        },
    })
    print(f"[seed] service:  {service['fullyQualifiedName']}")

    database = put("/v1/databases", {
        "name": "demo_db",
        "service": service["fullyQualifiedName"],
    })
    print(f"[seed] database: {database['fullyQualifiedName']}")

    schema = put("/v1/databaseSchemas", {
        "name": "public",
        "database": database["fullyQualifiedName"],
    })
    print(f"[seed] schema:   {schema['fullyQualifiedName']}")

    tables = {}
    for spec in [
        {
            "name": "orders",
            "description": "All completed orders from the checkout pipeline.",
            "columns": [
                {"name": "id", "dataType": "BIGINT", "constraint": "PRIMARY_KEY"},
                {"name": "customer_id", "dataType": "BIGINT", "description": "FK -> customers.id"},
                {"name": "total_amount", "dataType": "DECIMAL", "description": "Order total in USD"},
                {"name": "status", "dataType": "VARCHAR", "dataLength": 32},
                {"name": "created_at", "dataType": "TIMESTAMP"},
            ],
        },
        {
            "name": "customers",
            "description": "Customer dimension. One row per customer.",
            "columns": [
                {"name": "id", "dataType": "BIGINT", "constraint": "PRIMARY_KEY"},
                {"name": "email", "dataType": "VARCHAR", "dataLength": 255},
                {"name": "country", "dataType": "VARCHAR", "dataLength": 2},
                {"name": "signup_date", "dataType": "DATE"},
            ],
        },
        {
            "name": "daily_revenue",
            "description": "Per-day revenue rollup. Materialized from orders + customers.",
            "columns": [
                {"name": "day", "dataType": "DATE", "constraint": "PRIMARY_KEY"},
                {"name": "revenue_usd", "dataType": "DECIMAL"},
                {"name": "order_count", "dataType": "BIGINT"},
            ],
        },
    ]:
        body = {
            "name": spec["name"],
            "databaseSchema": schema["fullyQualifiedName"],
            "description": spec["description"],
            "columns": spec["columns"],
        }
        table = put("/v1/tables", body)
        tables[spec["name"]] = table
        print(f"[seed] table:    {table['fullyQualifiedName']}  ({len(spec['columns'])} columns)")

    # Add lineage: orders -> daily_revenue, customers -> daily_revenue
    for src in ["orders", "customers"]:
        put("/v1/lineage", {
            "edge": {
                "fromEntity": {"id": tables[src]["id"], "type": "table"},
                "toEntity":   {"id": tables["daily_revenue"]["id"], "type": "table"},
                "lineageDetails": {
                    "description": f"daily_revenue derives from {src}",
                },
            }
        })
        print(f"[seed] lineage:  {src} -> daily_revenue")

    # Allow the elastic indexer a moment to ingest.
    time.sleep(3)
    print("[seed] done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
