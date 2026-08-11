import importlib.util
import io
import json
import urllib.error
import urllib.parse
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = (
    ROOT
    / "spikes"
    / "precision"
    / "infra-hyperstack"
    / "tailscale-auth-key.py"
)
SPEC = importlib.util.spec_from_file_location("tailscale_auth_key", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class Response:
    def __init__(self, value=None, *, status=200, raw=None):
        self.status = status
        self.payload = raw if raw is not None else json.dumps(value).encode()

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return None

    def read(self, limit=-1):
        return self.payload if limit < 0 else self.payload[:limit]


class SequenceOpener:
    def __init__(self, *responses):
        self.responses = list(responses)
        self.requests = []

    def __call__(self, request, timeout):
        self.requests.append((request, timeout))
        response = self.responses.pop(0)
        if isinstance(response, BaseException):
            raise response
        return response


class TailscaleAuthKeyTest(unittest.TestCase):
    def test_create_uses_exact_scoped_one_off_contract(self):
        opener = SequenceOpener(
            Response(
                {
                    "access_token": "access-token-do-not-print",
                    "token_type": "Bearer",
                    "expires_in": 3600,
                    "scope": "auth_keys",
                }
            ),
            Response(
                {
                    "id": "key123",
                    "key": "tskey-auth-publicpart-secretpart",
                }
            ),
        )
        result = MODULE.create_auth_key(
            "oauth-client-id",
            "tskey-client-oauth-secret",
            tag="tag:sembla-bench",
            expiry_seconds=3600,
            description="sembla-hyperstack-test",
            urlopen=opener,
        )
        self.assertEqual(result["id"], "key123")
        self.assertEqual(result["key"], "tskey-auth-publicpart-secretpart")
        self.assertEqual(len(opener.requests), 2)

        token_request, token_timeout = opener.requests[0]
        self.assertEqual(token_timeout, 30)
        self.assertEqual(
            token_request.full_url,
            "https://api.tailscale.com/api/v2/oauth/token",
        )
        self.assertEqual(token_request.method, "POST")
        token_form = urllib.parse.parse_qs(token_request.data.decode())
        self.assertEqual(token_form["client_id"], ["oauth-client-id"])
        self.assertEqual(
            token_form["client_secret"], ["tskey-client-oauth-secret"]
        )
        self.assertEqual(token_form["grant_type"], ["client_credentials"])
        self.assertEqual(token_form["scope"], ["auth_keys"])

        key_request, key_timeout = opener.requests[1]
        self.assertEqual(key_timeout, 30)
        self.assertEqual(
            key_request.full_url,
            "https://api.tailscale.com/api/v2/tailnet/-/keys",
        )
        self.assertEqual(key_request.method, "POST")
        self.assertEqual(
            key_request.get_header("Authorization"),
            "Bearer access-token-do-not-print",
        )
        body = json.loads(key_request.data)
        self.assertEqual(
            body,
            {
                "capabilities": {
                    "devices": {
                        "create": {
                            "reusable": False,
                            "ephemeral": True,
                            "preauthorized": True,
                            "tags": ["tag:sembla-bench"],
                        }
                    }
                },
                "expirySeconds": 3600,
                "description": "sembla-hyperstack-test",
            },
        )
        serialized_key_request = (
            key_request.full_url
            + str(dict(key_request.header_items()))
            + key_request.data.decode()
        )
        self.assertNotIn("tskey-client-oauth-secret", serialized_key_request)

    def test_delete_is_verified_by_not_found(self):
        not_found = urllib.error.HTTPError(
            "https://api.tailscale.com/api/v2/tailnet/-/keys/key123",
            404,
            "not found",
            None,
            None,
        )
        opener = SequenceOpener(
            Response({"access_token": "access", "token_type": "Bearer", "scope": "auth_keys"}),
            Response(raw=b"", status=204),
            not_found,
        )
        MODULE.delete_auth_key(
            "client",
            "tskey-client-secret",
            key_id="key123",
            urlopen=opener,
        )
        self.assertEqual(opener.requests[1][0].method, "DELETE")
        self.assertEqual(opener.requests[2][0].method, "GET")

    def test_creation_transport_failure_is_ambiguous_and_redacted(self):
        opener = SequenceOpener(
            Response({"access_token": "access", "token_type": "Bearer", "scope": "auth_keys"}),
            urllib.error.URLError("network failed after send"),
        )
        with self.assertRaises(MODULE.AmbiguousKeyCreationError) as caught:
            MODULE.create_auth_key(
                "client",
                "tskey-client-super-secret",
                tag="tag:sembla-bench",
                expiry_seconds=3600,
                description="unique-description",
                urlopen=opener,
            )
        message = str(caught.exception)
        self.assertIn("do not retry", message)
        self.assertIn("unique-description", message)
        self.assertNotIn("super-secret", message)
        self.assertNotIn("access", message)

    def test_token_failure_does_not_include_response_or_secret(self):
        error = urllib.error.HTTPError(
            "https://api.tailscale.com/api/v2/oauth/token",
            401,
            "body contains tskey-client-secret",
            None,
            io.BytesIO(b"tskey-client-secret"),
        )
        with self.assertRaises(MODULE.TailscaleCredentialError) as caught:
            MODULE.create_auth_key(
                "client",
                "tskey-client-secret",
                tag="tag:sembla-bench",
                expiry_seconds=3600,
                description="description",
                urlopen=SequenceOpener(error),
            )
        self.assertEqual(
            str(caught.exception),
            "Tailscale OAuth token request failed with HTTP 401",
        )

    def test_invalid_created_key_is_deleted_before_failure(self):
        not_found = urllib.error.HTTPError(
            "https://api.tailscale.com/api/v2/tailnet/-/keys/key123",
            404,
            "not found",
            None,
            None,
        )
        opener = SequenceOpener(
            Response({"access_token": "access", "token_type": "Bearer", "scope": "auth_keys"}),
            Response({"id": "key123", "key": "not-an-auth-key"}),
            Response(raw=b"", status=204),
            not_found,
        )
        with self.assertRaisesRegex(
            MODULE.TailscaleCredentialError, "invalid auth key; the key was deleted"
        ):
            MODULE.create_auth_key(
                "client",
                "tskey-client-secret",
                tag="tag:sembla-bench",
                expiry_seconds=3600,
                description="description",
                urlopen=opener,
            )
        self.assertEqual(opener.requests[2][0].method, "DELETE")
        self.assertEqual(opener.requests[3][0].method, "GET")

    def test_credentials_use_nul_framing(self):
        self.assertEqual(
            MODULE._read_credentials(io.BytesIO(b"client\0secret\0")),
            ("client", "secret"),
        )
        with self.assertRaisesRegex(
            MODULE.TailscaleCredentialError, "two NUL-terminated fields"
        ):
            MODULE._read_credentials(io.BytesIO(b"client\nsecret\n"))

    def test_oauth_token_must_be_restricted_to_auth_keys(self):
        opener = SequenceOpener(
            Response(
                {
                    "access_token": "access",
                    "token_type": "Bearer",
                    "scope": "auth_keys devices:core",
                }
            )
        )
        with self.assertRaisesRegex(
            MODULE.TailscaleCredentialError, "not restricted to auth_keys"
        ):
            MODULE.create_auth_key(
                "client",
                "tskey-client-secret",
                tag="tag:sembla-bench",
                expiry_seconds=3600,
                description="description",
                urlopen=opener,
            )
        self.assertEqual(len(opener.requests), 1)

    def test_validation_rejects_broad_or_unbounded_inputs(self):
        opener = SequenceOpener()
        with self.assertRaisesRegex(MODULE.TailscaleCredentialError, "tag:name"):
            MODULE.create_auth_key(
                "client",
                "secret",
                tag="autogroup:admin",
                expiry_seconds=3600,
                description="description",
                urlopen=opener,
            )
        with self.assertRaisesRegex(MODULE.TailscaleCredentialError, "300 to 3600"):
            MODULE.create_auth_key(
                "client",
                "secret",
                tag="tag:sembla-bench",
                expiry_seconds=86_400,
                description="description",
                urlopen=opener,
            )
        self.assertFalse(opener.requests)


if __name__ == "__main__":
    unittest.main()
