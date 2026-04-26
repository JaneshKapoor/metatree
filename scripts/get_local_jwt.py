"""Retrieve the ingestion-bot JWT from a self-hosted OpenMetadata instance.

Usage:
    python scripts/get_local_jwt.py
    python scripts/get_local_jwt.py --host http://localhost:8585/api --write-env

Flow:
    1. POST /api/v1/users/login with the default admin credentials
       (admin@open-metadata.org / admin) — works on a fresh install with
       AUTHENTICATION_PROVIDER=basic, which is what our docker-compose.yml sets.
    2. GET /api/v1/users/name/ingestion-bot?fields=id  to find the bot's userId.
    3. GET /api/v1/users/auth-mechanism/{userId}  to read the JWT it was
       provisioned with.

If the admin account doesn't exist yet (fresh basic-auth install), step 1 will
return 401; in that case the script registers admin@open-metadata.org via
POST /api/v1/users/signup, then retries.

This avoids the UI-click flow documented at
https://docs.open-metadata.org/v1.12.x/deployment/security/enable-jwt-tokens
which is the only path the docs cover.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import re
import sys
import time
from pathlib import Path

import requests

DEFAULT_HOST = os.environ.get("OPENMETADATA_HOST", "http://localhost:8585/api")
ADMIN_EMAIL = "admin@open-metadata.org"
ADMIN_PASSWORD = "admin"


def b64(value: str) -> str:
    return base64.b64encode(value.encode("utf-8")).decode("ascii")


def wait_until_healthy(host: str, deadline_s: int = 180) -> None:
    """Poll /api/v1/system/version until the server responds."""
    base = host.rstrip("/")
    deadline = time.monotonic() + deadline_s
    last_err: Exception | None = None
    while time.monotonic() < deadline:
        try:
            response = requests.get(f"{base}/v1/system/version", timeout=5)
            if response.ok:
                return
        except requests.RequestException as exc:
            last_err = exc
        time.sleep(3)
    raise SystemExit(
        f"OpenMetadata at {host} did not become reachable within {deadline_s}s "
        f"(last error: {last_err})"
    )


def login(host: str, email: str, password: str) -> str:
    """POST /api/v1/users/login. Returns access JWT or empty string on failure."""
    response = requests.post(
        f"{host.rstrip('/')}/v1/users/login",
        json={"email": email, "password": b64(password)},
        timeout=15,
    )
    if response.status_code == 200:
        return str(response.json().get("accessToken") or "")
    return ""


def signup_admin(host: str) -> bool:
    """Register the default admin user. Idempotent: 4xx with "already exists" is fine."""
    response = requests.post(
        f"{host.rstrip('/')}/v1/users/signup",
        json={
            "email": ADMIN_EMAIL,
            "firstName": "Admin",
            "lastName": "Admin",
            "password": ADMIN_PASSWORD,
        },
        timeout=15,
    )
    return response.status_code in (200, 201, 409)


def fetch_bot_jwt(host: str, access_token: str, bot_name: str = "ingestion-bot") -> str:
    """Retrieve the JWT auth-mechanism for a bot, returning the raw token."""
    headers = {"Authorization": f"Bearer {access_token}"}
    bot_user = requests.get(
        f"{host.rstrip('/')}/v1/users/name/{bot_name}",
        params={"fields": "id"},
        headers=headers,
        timeout=15,
    )
    if not bot_user.ok:
        raise SystemExit(f"could not GET bot user: {bot_user.status_code} {bot_user.text[:200]}")
    bot_id = bot_user.json().get("id")
    if not bot_id:
        raise SystemExit("bot user response missing `id` field")
    auth = requests.get(
        f"{host.rstrip('/')}/v1/users/auth-mechanism/{bot_id}",
        headers=headers,
        timeout=15,
    )
    if not auth.ok:
        raise SystemExit(f"could not GET auth-mechanism: {auth.status_code} {auth.text[:200]}")
    config = auth.json().get("config") or {}
    token = config.get("JWTToken") or config.get("jwtToken") or config.get("token")
    if not token:
        raise SystemExit(
            f"auth-mechanism response did not contain a JWT: {json.dumps(auth.json())[:300]}"
        )
    return str(token)


def write_env(env_path: Path, host: str, token: str) -> None:
    """Update or insert OPENMETADATA_HOST and OPENMETADATA_JWT_TOKEN in `.env`."""
    text = env_path.read_text(encoding="utf-8") if env_path.exists() else ""
    text = upsert(text, "OPENMETADATA_HOST", host)
    text = upsert(text, "OPENMETADATA_JWT_TOKEN", token)
    env_path.write_text(text, encoding="utf-8")


def upsert(body: str, key: str, value: str) -> str:
    line = f"{key}={value}"
    pattern = re.compile(rf"^{re.escape(key)}=.*$", re.M)
    if pattern.search(body):
        return pattern.sub(line, body)
    if body and not body.endswith("\n"):
        body += "\n"
    return body + line + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default=DEFAULT_HOST,
                        help="OpenMetadata API base URL (default: env OPENMETADATA_HOST)")
    parser.add_argument("--write-env", action="store_true",
                        help="Persist host + token into .env at the repo root")
    parser.add_argument("--env-path", default=".env",
                        help="Path to the .env file to update (default: ./.env)")
    parser.add_argument("--wait", type=int, default=180,
                        help="Seconds to wait for the server to become reachable")
    args = parser.parse_args(argv)

    host = args.host.rstrip("/")
    print(f"[get_local_jwt] waiting for {host} (up to {args.wait}s)...", flush=True)
    wait_until_healthy(host, deadline_s=args.wait)

    print("[get_local_jwt] logging in as admin...", flush=True)
    token = login(host, ADMIN_EMAIL, ADMIN_PASSWORD)
    if not token:
        print("[get_local_jwt] admin login failed -- attempting signup then retrying", flush=True)
        signup_admin(host)
        token = login(host, ADMIN_EMAIL, ADMIN_PASSWORD)
    if not token:
        raise SystemExit(
            "could not obtain an admin access token. Try logging in via the UI "
            "at http://localhost:8585 (admin@open-metadata.org / admin) once, then retry."
        )

    print("[get_local_jwt] fetching ingestion-bot auth mechanism...", flush=True)
    jwt = fetch_bot_jwt(host, token)
    print("\n=== INGESTION-BOT JWT ===")
    print(jwt)
    print("=========================\n")

    if args.write_env:
        env_path = Path(args.env_path)
        write_env(env_path, host, jwt)
        print(f"[get_local_jwt] wrote OPENMETADATA_HOST and OPENMETADATA_JWT_TOKEN to {env_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
