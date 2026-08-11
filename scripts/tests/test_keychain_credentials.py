import os
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = (
    ROOT
    / "spikes"
    / "precision"
    / "infra-hyperstack"
    / "keychain-credentials.sh"
)


class KeychainCredentialsTest(unittest.TestCase):
    def test_shell_syntax_and_help(self):
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)
        result = subprocess.run(
            ["bash", str(SCRIPT), "--help"],
            text=True,
            capture_output=True,
            check=True,
        )
        self.assertIn("prepare-shell", result.stdout)
        self.assertIn("exec COMMAND", result.stdout)

    def mock_environment(self, root):
        mock_bin = root / "bin"
        mock_bin.mkdir()
        security_log = root / "security.log"
        security = mock_bin / "security"
        security.write_text(
            "#!/usr/bin/env bash\n"
            "printf '%q ' \"$@\" >> \"$MOCK_SECURITY_LOG\"; printf '\\n' >> \"$MOCK_SECURITY_LOG\"\n"
            "if [[ $1 == find-generic-password ]]; then\n"
            "  service=''\n"
            "  while (( $# )); do\n"
            "    [[ $1 == -s ]] && { service=$2; break; }; shift\n"
            "  done\n"
            "  case $service in\n"
            "    sembla.hyperstack.api-key) printf hyperstack-test-key ;;\n"
            "    sembla.tailscale.oauth-client-id) printf oauth-client-id ;;\n"
            "    sembla.tailscale.oauth-client-secret) printf tskey-client-test-secret ;;\n"
            "    *) exit 44 ;;\n"
            "  esac\n"
            "fi\n"
        )
        security.chmod(0o755)
        launchctl = mock_bin / "launchctl"
        launchctl.write_text(
            "#!/usr/bin/env bash\n"
            "printf '%s|%s\\n' \"${HYPERSTACK_API_KEY-unset}\" \"$*\" "
            ">> \"$MOCK_LAUNCHCTL_LOG\"\n"
            "if [[ $1 == getenv ]]; then\n"
            "  case $2 in\n"
            "    TF_VAR_tailscale_auth_key) printf tskey-auth-session-key ;;\n"
            "    SSH_HOST_KEY_FINGERPRINT) printf SHA256:test ;;\n"
            "    SEMBLA_TAILSCALE_AUTH_KEY_ID) printf key123 ;;\n"
            "  esac\n"
            "fi\n"
        )
        launchctl.chmod(0o755)
        environment = os.environ.copy()
        environment["PATH"] = f"{mock_bin}:{environment['PATH']}"
        environment["MOCK_SECURITY_LOG"] = str(security_log)
        environment["MOCK_LAUNCHCTL_LOG"] = str(root / "launchctl.log")
        environment["USER"] = "test-user"
        return environment, security_log

    def test_check_validates_without_printing_credentials(self):
        with tempfile.TemporaryDirectory() as directory:
            environment, _log = self.mock_environment(Path(directory))
            result = subprocess.run(
                ["bash", str(SCRIPT), "check"],
                env=environment,
                text=True,
                capture_output=True,
                check=True,
            )
            combined = result.stdout + result.stderr
            self.assertIn("structurally valid", combined)
            self.assertNotIn("hyperstack-test-key", combined)
            self.assertNotIn("tskey-client-test-secret", combined)

    def test_exec_injects_api_key_and_prepared_session_into_child_only(self):
        with tempfile.TemporaryDirectory() as directory:
            environment, _log = self.mock_environment(Path(directory))
            environment["HYPERSTACK_API_KEY"] = "inherited-provider-key"
            result = subprocess.run(
                [
                    "bash",
                    str(SCRIPT),
                    "exec",
                    "bash",
                    "-c",
                    "printf '%s|%s|%s' \"$HYPERSTACK_API_KEY\" \"$TF_VAR_tailscale_auth_key\" \"$SEMBLA_TAILSCALE_AUTH_KEY_ID\"",
                ],
                env=environment,
                text=True,
                capture_output=True,
                check=True,
            )
            self.assertEqual(
                result.stdout,
                "hyperstack-test-key|tskey-auth-session-key|key123",
            )
            launchctl_calls = (Path(directory) / "launchctl.log").read_text().splitlines()
            self.assertTrue(launchctl_calls)
            self.assertTrue(
                all(call.startswith("unset|") for call in launchctl_calls),
                launchctl_calls,
            )

    def test_store_never_places_secret_in_argv_and_uses_prompting_w_last(self):
        with tempfile.TemporaryDirectory() as directory:
            environment, log = self.mock_environment(Path(directory))
            result = subprocess.run(
                ["bash", str(SCRIPT), "store"],
                env=environment,
                text=True,
                capture_output=True,
                check=True,
            )
            calls = log.read_text().splitlines()
            add_calls = [line for line in calls if line.startswith("add-generic-password")]
            self.assertEqual(len(add_calls), 3)
            for call in add_calls:
                self.assertTrue(call.endswith("-w "), call)
                self.assertNotIn(" -A ", f" {call} ")
            log_text = log.read_text()
            self.assertNotIn("hyperstack-test-key", log_text)
            self.assertNotIn("tskey-client-test-secret", log_text)

    def test_source_keeps_oauth_out_of_child_environment_and_launchctl(self):
        source = SCRIPT.read_text()
        prepare = source[source.index("prepare_shell() {") : source.index("\ncleanup_session() {")]
        self.assertIn("prepare-paid-session.sh\" --tailscale-oauth-keychain", source)
        self.assertLess(
            prepare.index("prepare-paid-session.sh"),
            prepare.index("import_session"),
        )
        self.assertLess(
            prepare.index("import_session"),
            prepare.index("export HYPERSTACK_API_KEY"),
        )
        self.assertLess(
            prepare.index("unset HYPERSTACK_API_KEY"),
            prepare.index('read_item "$HYPERSTACK_SERVICE"'),
        )
        self.assertLess(
            prepare.index("unset HYPERSTACK_API_KEY"),
            prepare.index("prepare-paid-session.sh"),
        )
        self.assertNotIn("load_hyperstack", prepare)
        shell_case = source[source.index("  shell)") : source.index("  exec)")]
        exec_case = source[source.index("  exec)") : source.index("  cleanup-session)")]
        for case in (shell_case, exec_case):
            self.assertLess(
                case.index("unset HYPERSTACK_API_KEY"), case.index("import_session")
            )
            self.assertLess(
                case.index("import_session"), case.index("load_hyperstack")
            )
        self.assertNotIn("export TAILSCALE_OAUTH", source)
        self.assertNotIn("launchctl setenv HYPERSTACK_API_KEY", source)
        self.assertLess(source.index("set +x"), source.index("set -Eeuo pipefail"))

    def test_terraform_accepts_only_disposable_auth_keys_and_requires_one_paid(self):
        variables = (
            ROOT / "spikes/precision/infra-hyperstack/variables.tf"
        ).read_text()
        main = (ROOT / "spikes/precision/infra-hyperstack/main.tf").read_text()
        self.assertIn('regex("^tskey-auth-', variables)
        self.assertNotIn('regex("^tskey-(auth|client)', variables)
        self.assertIn("never pass a tskey-client OAuth secret", variables)
        self.assertGreaterEqual(
            main.count("nonsensitive(length(var.tailscale_auth_key)) > 0"), 2
        )

    def test_cloud_init_joins_tailnet_before_build_and_ready_and_fails_closed(self):
        source = (
            ROOT / "spikes/precision/infra-hyperstack/cloud-init.sh.tftpl"
        ).read_text()
        join = source.index(
            'tailscale up --auth-key="file:$TAILSCALE_AUTH_KEY_FILE"'
        )
        self.assertLess(join, source.index("git clone"))
        self.assertLess(join, source.index("cargo build"))
        self.assertLess(join, source.index('> "$STATUS_DIR/ready"'))
        self.assertIn("Tailscale registration failed", source)
        self.assertNotIn("public-IP path remains", source)
        self.assertNotIn('tailscale up --authkey="$TAILSCALE_AUTH_KEY"', source)
        self.assertIn("mktemp /run/sembla-tailscale-auth.XXXXXX", source)
        self.assertIn('chmod 0600 "$TAILSCALE_AUTH_KEY_FILE"', source)
        self.assertIn('rm -f -- "$TAILSCALE_AUTH_KEY_FILE"', source)
        self.assertNotIn("TAILSCALE_OAUTH_CLIENT_SECRET", source)

    def test_cleanup_refuses_while_paid_state_or_provider_orphans_exist(self):
        source = SCRIPT.read_text()
        state_check = source.index("terraform state list")
        reconcile = source.index("bash reconcile-orphans.sh")
        revoke = source.index('"$TAILSCALE_AUTH_KEY_HELPER" delete')
        clear = source.index('launchctl unsetenv "$name"')
        self.assertLess(state_check, reconcile)
        self.assertLess(reconcile, revoke)
        cleanup = source[
            source.index("cleanup_session() {") : source.index("\n[[ $# -ge 1 ]]")
        ]
        self.assertLess(
            cleanup.index("unset HYPERSTACK_API_KEY"),
            cleanup.index("terraform state list"),
        )
        self.assertIn("exec bash reconcile-orphans.sh", cleanup)
        self.assertLess(revoke, clear)
        self.assertIn("paid VM/security-rule state still exists", source)
        self.assertIn("provider reconciliation is not clean", source)
        self.assertIn('"$MODULE_DIR"/.host-key-*', source)
        self.assertNotIn("delete-generic-password", source)


if __name__ == "__main__":
    unittest.main()
