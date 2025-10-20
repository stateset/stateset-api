#!/usr/bin/env python3

"""
Generate a local admin JWT without compiling any Rust binaries.

Usage:
  python3 scripts/dev_admin_token.py

The script looks for a JWT secret in the following order:
  1. APP__JWT_SECRET environment variable
  2. JWT_SECRET environment variable
  3. jwt_secret value from config/default.toml
"""

from __future__ import annotations

import base64
import hashlib
import hmac
import json
import os
import sys
import time
import uuid

try:
    import tomllib  # Python 3.11+
except ModuleNotFoundError:  # pragma: no cover - fallback for older Python
    import tomli as tomllib  # type: ignore


DEFAULT_PERMISSIONS = [
    # Orders
    "orders:read",
    "orders:create",
    "orders:update",
    "orders:delete",
    "orders:cancel",
    "orders:*",
    # Inventory
    "inventory:read",
    "inventory:adjust",
    "inventory:transfer",
    "inventory:*",
    # Returns
    "returns:read",
    "returns:create",
    "returns:approve",
    "returns:reject",
    "returns:*",
    # Shipments
    "shipments:read",
    "shipments:create",
    "shipments:update",
    "shipments:delete",
    "shipments:*",
    # Warranties
    "warranties:read",
    "warranties:create",
    "warranties:update",
    "warranties:delete",
    "warranties:*",
    # Work orders
    "workorders:read",
    "workorders:create",
    "workorders:update",
    "workorders:delete",
    "workorders:*",
    # Misc application permissions
    "admin:outbox",
    "payments:access",
    "agents:access",
]


def load_config_value(key: str) -> str | None:
    # Env vars take precedence
    env_map = {
        "jwt_secret": ("APP__JWT_SECRET", "JWT_SECRET"),
        "jwt_expiration": ("APP__JWT_EXPIRATION", "JWT_EXPIRATION"),
    }
    for candidate in env_map.get(key, ()):
        val = os.environ.get(candidate)
        if val:
            return val

    # Fall back to config/default.toml
    try:
        with open("config/default.toml", "rb") as fh:
            cfg = tomllib.load(fh)
            val = cfg.get(key)
            if val is not None:
                return str(val)
    except FileNotFoundError:
        pass

    return None


def b64url(data: bytes) -> bytes:
    return base64.urlsafe_b64encode(data).rstrip(b"=")


def main() -> int:
    secret = load_config_value("jwt_secret")
    if not secret:
        print("Unable to locate JWT secret. Set APP__JWT_SECRET or update config/default.toml.",
              file=sys.stderr)
        return 1

    try:
        exp_seconds = int(load_config_value("jwt_expiration") or "3600")
    except ValueError:
        exp_seconds = 3600

    now = int(time.time())
    exp = now + exp_seconds

    claims = {
        "sub": str(uuid.uuid4()),
        "name": "Local Admin",
        "email": "admin@example.com",
        "roles": ["admin"],
        "permissions": DEFAULT_PERMISSIONS,
        "tenant_id": None,
        "jti": str(uuid.uuid4()),
        "iat": now,
        "exp": exp,
        "nbf": now,
        "iss": "stateset-auth",
        "aud": "stateset-api",
        "scope": None,
    }

    header = {"alg": "HS256", "typ": "JWT"}

    header_b64 = b64url(json.dumps(header, separators=(",", ":"), sort_keys=True).encode())
    payload_b64 = b64url(json.dumps(claims, separators=(",", ":"), sort_keys=True).encode())
    signing_input = header_b64 + b"." + payload_b64

    signature = hmac.new(
        secret.encode(),
        signing_input,
        digestmod=hashlib.sha256,
    ).digest()
    signature_b64 = b64url(signature)

    token = (signing_input + b"." + signature_b64).decode()

    print(f"Generated admin JWT (valid for {exp_seconds} seconds):\n")
    print(token)
    print("\nUse it as:")
    print(f"Authorization: Bearer {token}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
