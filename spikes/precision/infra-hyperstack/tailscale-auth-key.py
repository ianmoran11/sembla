#!/usr/bin/env python3
"""Mint or revoke one short-lived Tailscale auth key without exposing OAuth secrets.

OAuth client credentials are read from stdin as two NUL-terminated UTF-8 fields:
client ID, then client secret. Successful ``create`` writes one JSON object to
stdout. Errors never include API response bodies or credential values.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from typing import Any, Callable


API_BASE = "https://api.tailscale.com"
MAX_RESPONSE_BYTES = 1_048_576
AUTH_KEY_RE = re.compile(r"^tskey-auth-[A-Za-z0-9-]+$")
KEY_ID_RE = re.compile(r"^[A-Za-z0-9_-]+$")
TAG_RE = re.compile(r"^tag:[A-Za-z0-9][A-Za-z0-9-]*$")
UrlOpen = Callable[..., Any]


class TailscaleCredentialError(RuntimeError):
    """A deterministic credential or API-contract failure."""


class AmbiguousKeyCreationError(TailscaleCredentialError):
    """The create request may have reached Tailscale but returned no result."""


class _Response:
    def __init__(self, payload: bytes, status: int = 200):
        self.payload = payload
        self.status = status

    def __enter__(self) -> "_Response":
        return self

    def __exit__(self, *_args: object) -> None:
        return None

    def read(self, limit: int = -1) -> bytes:
        return self.payload if limit < 0 else self.payload[:limit]


def _read_credentials(stream: Any = sys.stdin.buffer) -> tuple[str, str]:
    raw = stream.read(65_537)
    if len(raw) > 65_536:
        raise TailscaleCredentialError("OAuth credential input exceeds 64 KiB")
    fields = raw.split(b"\0")
    if len(fields) != 3 or fields[-1] != b"":
        raise TailscaleCredentialError(
            "OAuth credentials must be two NUL-terminated fields on stdin"
        )
    try:
        client_id, client_secret = (field.decode("utf-8") for field in fields[:2])
    except UnicodeDecodeError as error:
        raise TailscaleCredentialError("OAuth credentials must be UTF-8") from error
    if not client_id or not client_secret:
        raise TailscaleCredentialError("OAuth client ID and secret must be non-empty")
    if any(character.isspace() for character in client_id + client_secret):
        raise TailscaleCredentialError("OAuth credentials must not contain whitespace")
    return client_id, client_secret


def _read_json(response: Any, phase: str) -> dict[str, Any]:
    payload = response.read(MAX_RESPONSE_BYTES + 1)
    if len(payload) > MAX_RESPONSE_BYTES:
        raise TailscaleCredentialError(f"Tailscale {phase} response exceeds 1 MiB")
    try:
        value = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise TailscaleCredentialError(
            f"Tailscale {phase} returned malformed JSON"
        ) from error
    if not isinstance(value, dict):
        raise TailscaleCredentialError(
            f"Tailscale {phase} returned a non-object JSON value"
        )
    return value


def _open_json(
    request: urllib.request.Request,
    phase: str,
    *,
    urlopen: UrlOpen = urllib.request.urlopen,
) -> dict[str, Any]:
    try:
        with urlopen(request, timeout=30) as response:
            status = getattr(response, "status", 200)
            if status < 200 or status >= 300:
                raise TailscaleCredentialError(
                    f"Tailscale {phase} failed with HTTP {status}"
                )
            return _read_json(response, phase)
    except urllib.error.HTTPError as error:
        raise TailscaleCredentialError(
            f"Tailscale {phase} failed with HTTP {error.code}"
        ) from None
    except urllib.error.URLError as error:
        raise TailscaleCredentialError(
            f"Tailscale {phase} could not reach the API"
        ) from None
    except TimeoutError:
        raise TailscaleCredentialError(f"Tailscale {phase} timed out") from None


def _oauth_access_token(
    client_id: str,
    client_secret: str,
    *,
    urlopen: UrlOpen = urllib.request.urlopen,
) -> str:
    form = urllib.parse.urlencode(
        {
            "client_id": client_id,
            "client_secret": client_secret,
            "grant_type": "client_credentials",
            "scope": "auth_keys",
        }
    ).encode("ascii")
    request = urllib.request.Request(
        f"{API_BASE}/api/v2/oauth/token",
        data=form,
        headers={
            "Accept": "application/json",
            "Content-Type": "application/x-www-form-urlencoded",
            "User-Agent": "sembla-hyperstack-auth-key/1",
        },
        method="POST",
    )
    response = _open_json(request, "OAuth token request", urlopen=urlopen)
    token = response.get("access_token")
    token_type = response.get("token_type")
    scope = response.get("scope")
    if not isinstance(token, str) or not token:
        raise TailscaleCredentialError(
            "Tailscale OAuth token response omitted access_token"
        )
    if not isinstance(token_type, str) or token_type.lower() != "bearer":
        raise TailscaleCredentialError(
            "Tailscale OAuth token response did not declare Bearer"
        )
    if not isinstance(scope, str) or set(scope.split()) != {"auth_keys"}:
        raise TailscaleCredentialError(
            "Tailscale OAuth token was not restricted to auth_keys"
        )
    return token


def _key_request(
    method: str,
    access_token: str,
    key_id: str | None = None,
    body: dict[str, Any] | None = None,
) -> urllib.request.Request:
    suffix = "/keys"
    if key_id is not None:
        if not KEY_ID_RE.fullmatch(key_id):
            raise TailscaleCredentialError("Tailscale key ID has an invalid format")
        suffix += f"/{urllib.parse.quote(key_id, safe='')}"
    data = None if body is None else json.dumps(body, separators=(",", ":")).encode()
    headers = {
        "Accept": "application/json",
        "Authorization": f"Bearer {access_token}",
        "User-Agent": "sembla-hyperstack-auth-key/1",
    }
    if data is not None:
        headers["Content-Type"] = "application/json"
    return urllib.request.Request(
        f"{API_BASE}/api/v2/tailnet/-{suffix}",
        data=data,
        headers=headers,
        method=method,
    )


def _delete_with_token(
    access_token: str,
    key_id: str,
    *,
    urlopen: UrlOpen = urllib.request.urlopen,
) -> None:
    request = _key_request("DELETE", access_token, key_id)
    try:
        with urlopen(request, timeout=30) as response:
            status = getattr(response, "status", 204)
            if status < 200 or status >= 300:
                raise TailscaleCredentialError(
                    f"Tailscale auth-key deletion failed with HTTP {status}"
                )
    except urllib.error.HTTPError as error:
        if error.code != 404:
            raise TailscaleCredentialError(
                f"Tailscale auth-key deletion failed with HTTP {error.code}"
            ) from None
    except (urllib.error.URLError, TimeoutError):
        raise TailscaleCredentialError(
            "Tailscale auth-key deletion could not be confirmed"
        ) from None

    verify = _key_request("GET", access_token, key_id)
    try:
        with urlopen(verify, timeout=30) as response:
            status = getattr(response, "status", 200)
            if 200 <= status < 300:
                raise TailscaleCredentialError(
                    "Tailscale auth key still exists after deletion"
                )
            raise TailscaleCredentialError(
                f"Tailscale auth-key deletion verification returned HTTP {status}"
            )
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return
        raise TailscaleCredentialError(
            f"Tailscale auth-key deletion verification failed with HTTP {error.code}"
        ) from None
    except (urllib.error.URLError, TimeoutError):
        raise TailscaleCredentialError(
            "Tailscale auth-key deletion verification could not reach the API"
        ) from None


def create_auth_key(
    client_id: str,
    client_secret: str,
    *,
    tag: str,
    expiry_seconds: int,
    description: str,
    urlopen: UrlOpen = urllib.request.urlopen,
) -> dict[str, Any]:
    if not TAG_RE.fullmatch(tag):
        raise TailscaleCredentialError("Tailscale tag must look like tag:name")
    if not 300 <= expiry_seconds <= 3600:
        raise TailscaleCredentialError("auth-key expiry must be 300 to 3600 seconds")
    if not description or len(description) > 100:
        raise TailscaleCredentialError("description must contain 1 to 100 characters")

    access_token = _oauth_access_token(
        client_id, client_secret, urlopen=urlopen
    )
    body = {
        "capabilities": {
            "devices": {
                "create": {
                    "reusable": False,
                    "ephemeral": True,
                    "preauthorized": True,
                    "tags": [tag],
                }
            }
        },
        "expirySeconds": expiry_seconds,
        "description": description,
    }
    request = _key_request("POST", access_token, body=body)
    try:
        response = _open_json(request, "auth-key creation", urlopen=urlopen)
    except TailscaleCredentialError:
        # Once POST starts, even an HTTP/transport/parse failure is treated as
        # potentially committed. Never retry automatically and never print the
        # response body; the unique description plus short expiry bounds it.
        raise AmbiguousKeyCreationError(
            "Tailscale auth-key creation outcome is unknown; do not retry this "
            f"description ({description}); inspect Tailscale or wait "
            f"{expiry_seconds} seconds for expiry"
        ) from None

    key_id = response.get("id")
    auth_key = response.get("key")
    if not isinstance(key_id, str) or not KEY_ID_RE.fullmatch(key_id):
        raise AmbiguousKeyCreationError(
            "Tailscale created an unidentifiable auth key; do not retry; inspect "
            f"description {description} or wait {expiry_seconds} seconds for expiry"
        )
    if not isinstance(auth_key, str) or not AUTH_KEY_RE.fullmatch(auth_key):
        try:
            _delete_with_token(access_token, key_id, urlopen=urlopen)
        except TailscaleCredentialError as error:
            raise TailscaleCredentialError(
                "Tailscale returned an invalid auth key and cleanup was not verified"
            ) from error
        raise TailscaleCredentialError(
            "Tailscale returned an invalid auth key; the key was deleted"
        )
    return {
        "id": key_id,
        "key": auth_key,
        "expiry_seconds": expiry_seconds,
        "description": description,
    }


def delete_auth_key(
    client_id: str,
    client_secret: str,
    *,
    key_id: str,
    urlopen: UrlOpen = urllib.request.urlopen,
) -> None:
    access_token = _oauth_access_token(
        client_id, client_secret, urlopen=urlopen
    )
    _delete_with_token(access_token, key_id, urlopen=urlopen)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    create = subparsers.add_parser("create", help="mint one disposable auth key")
    create.add_argument("--tag", default="tag:sembla-bench")
    create.add_argument("--expiry-seconds", type=int, default=3600)
    create.add_argument("--description")
    delete = subparsers.add_parser("delete", help="revoke and verify one auth key")
    delete.add_argument("--key-id", required=True)
    return parser


def main() -> int:
    args = _parser().parse_args()
    try:
        client_id, client_secret = _read_credentials()
        if args.command == "create":
            description = args.description or (
                "sembla-hyperstack-"
                + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
            )
            result = create_auth_key(
                client_id,
                client_secret,
                tag=args.tag,
                expiry_seconds=args.expiry_seconds,
                description=description,
            )
            json.dump(result, sys.stdout, separators=(",", ":"))
            sys.stdout.write("\n")
        else:
            delete_auth_key(
                client_id, client_secret, key_id=args.key_id
            )
    except AmbiguousKeyCreationError as error:
        print(f"error: {error}", file=sys.stderr)
        return 75
    except TailscaleCredentialError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
