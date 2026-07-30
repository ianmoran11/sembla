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
    / "prepare-paid-session.sh"
)


class PreparePaidSessionTest(unittest.TestCase):
    def test_shell_syntax_and_help(self):
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)
        result = subprocess.run(
            ["bash", str(SCRIPT), "--help"],
            text=True,
            capture_output=True,
            check=True,
        )
        self.assertIn("Interactively prepares one fresh paid-run session", result.stdout)

    def test_prompts_secrets_without_echo_and_reuses_audited_helpers(self):
        source = SCRIPT.read_text()
        self.assertLess(source.index("set +x"), source.index("set -Eeuo pipefail"))
        self.assertIn("bash +x", source)
        self.assertIn("read -r -s TAILSCALE_AUTH_KEY < /dev/tty", source)
        self.assertIn('prepare-console-password.sh', source)
        self.assertIn('prepare-deploy-key.sh', source)
        self.assertIn('prepare-host-key.sh', source)
        self.assertNotIn('echo "$TAILSCALE_AUTH_KEY"', source)
        self.assertNotIn('echo "$TF_VAR_evidence_deploy_key"', source)
        self.assertNotIn('echo "$TF_VAR_console_password_hash"', source)

    def test_verifies_protection_and_registers_write_deploy_key(self):
        source = SCRIPT.read_text()
        self.assertIn("branches/main/protection", source)
        self.assertIn(".enforce_admins.enabled == true", source)
        self.assertIn(".allow_force_pushes.enabled == false", source)
        self.assertIn(".allow_deletions.enabled == false", source)
        self.assertIn('-F read_only=false', source)
        self.assertIn("SEMBLA_EVIDENCE_DEPLOY_KEY_ID", source)

    def test_rolls_back_partial_setup(self):
        source = SCRIPT.read_text()
        self.assertIn('if [[ "$SUCCESS" != 1 && "$MUTATED" == 1 ]]', source)
        self.assertIn("launchctl unsetenv", source)
        self.assertIn("gh api --method DELETE", source)
        self.assertIn("ROLLBACK WARNING", source)
        self.assertIn("launchctl could not verify removal", source)
        self.assertIn("GitHub deploy-key list verification failed", source)
        self.assertIn("remove_owned_host_dir", source)
        self.assertGreaterEqual(source.count("find_registered_deploy_key_id"), 3)
        self.assertEqual(
            source.count('remaining="$(find_registered_deploy_key_id)"'), 2
        )

    def test_existing_session_is_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            mock_bin = root / "bin"
            mock_bin.mkdir()
            log = root / "launchctl.log"
            launchctl = mock_bin / "launchctl"
            launchctl.write_text(
                "#!/usr/bin/env bash\n"
                "if [[ $1 == getenv && $2 == TF_VAR_tailscale_auth_key ]]; then\n"
                "  echo existing-secret\n"
                "elif [[ $1 == unsetenv ]]; then\n"
                "  echo \"$2\" >> \"$MOCK_LAUNCHCTL_LOG\"\n"
                "fi\n"
            )
            launchctl.chmod(0o755)
            gh = mock_bin / "gh"
            gh.write_text(
                "#!/usr/bin/env bash\n"
                "if [[ $1 == auth && $2 == status ]]; then exit 0; fi\n"
                "if [[ $1 == api ]]; then\n"
                "  printf '%s\\n' '{\"enforce_admins\":{\"enabled\":true},\"required_pull_request_reviews\":{},\"allow_force_pushes\":{\"enabled\":false},\"allow_deletions\":{\"enabled\":false}}'\n"
                "  exit 0\n"
                "fi\n"
                "exit 1\n"
            )
            gh.chmod(0o755)
            environment = os.environ.copy()
            environment["PATH"] = f"{mock_bin}:{environment['PATH']}"
            environment["MOCK_LAUNCHCTL_LOG"] = str(log)
            result = subprocess.run(
                ["bash", str(SCRIPT), "--repo", "owner/repo"],
                env=environment,
                text=True,
                capture_output=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("existing session value detected", result.stderr)
            self.assertFalse(log.exists(), "stale-session detection must not unset values")

    def test_launchctl_read_failure_stops_before_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            mock_bin = root / "bin"
            mock_bin.mkdir()
            log = root / "launchctl.log"
            launchctl = mock_bin / "launchctl"
            launchctl.write_text(
                "#!/usr/bin/env bash\n"
                "if [[ $1 == getenv ]]; then exit 71; fi\n"
                "echo \"$*\" >> \"$MOCK_LAUNCHCTL_LOG\"\n"
            )
            launchctl.chmod(0o755)
            gh = mock_bin / "gh"
            gh.write_text(
                "#!/usr/bin/env bash\n"
                "if [[ $1 == auth && $2 == status ]]; then exit 0; fi\n"
                "printf '%s\\n' '{\"enforce_admins\":{\"enabled\":true},\"required_pull_request_reviews\":{},\"allow_force_pushes\":{\"enabled\":false},\"allow_deletions\":{\"enabled\":false}}'\n"
            )
            gh.chmod(0o755)
            environment = os.environ.copy()
            environment["PATH"] = f"{mock_bin}:{environment['PATH']}"
            environment["MOCK_LAUNCHCTL_LOG"] = str(log)
            result = subprocess.run(
                ["bash", str(SCRIPT), "--repo", "owner/repo"],
                env=environment,
                text=True,
                capture_output=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("could not inspect existing launchctl value", result.stderr)
            self.assertFalse(log.exists(), "read failure must stop before mutation")

    def test_sets_and_verifies_all_session_values(self):
        source = SCRIPT.read_text()
        names = (
            "TF_VAR_tailscale_auth_key",
            "TF_VAR_console_password_hash",
            "TF_VAR_evidence_deploy_key",
            "TF_VAR_ssh_host_private_key",
            "SSH_HOST_KEY_FINGERPRINT",
            "SEMBLA_HOST_KEY_DIR",
            "SEMBLA_EVIDENCE_DEPLOY_KEY_ID",
        )
        for name in names:
            with self.subTest(name=name):
                self.assertIn(f"set_launchctl_value {name}", source)
        self.assertIn('if ! actual="$(launchctl getenv "$name")"', source)
        self.assertIn('[[ "$actual" == "$expected" ]]', source)

    def test_setenv_readback_failure_is_not_accepted(self):
        source = SCRIPT.read_text()
        start = source.index("set_launchctl_value() {")
        end = source.index("\n}\nset_launchctl_value TF_VAR_tailscale_auth_key", start) + 2
        function = source[start:end]
        script = f"""set -Eeuo pipefail
{function}
launchctl() {{
  if [[ $1 == setenv ]]; then return 0; fi
  if [[ $1 == getenv ]]; then printf '%s\\n' expected; return 71; fi
}}
set +e
set_launchctl_value test-name expected
rc=$?
set -e
printf 'rc=%s\\n' "$rc"
[[ "$rc" == 1 ]]
"""
        result = subprocess.run(["bash", "-c", script], text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("rc=1", result.stdout)
        self.assertIn("launchctl readback failed", result.stderr)

    def test_ambiguous_post_key_is_reconciled_deleted_and_relisted(self):
        source = SCRIPT.read_text()
        start = source.index("find_registered_deploy_key_id() {")
        end = source.index("\ntrap cleanup EXIT", start)
        functions = source[start:end]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            counter = root / "counter"
            delete_log = root / "delete.log"
            counter.write_text("0")
            script = f"""set -Eeuo pipefail
MODULE_DIR={root!s}
GITHUB_REPOSITORY=owner/repo
SUCCESS=0
MUTATED=1
REGISTERED_DEPLOY_KEY_ID=''
DEPLOY_PUBLIC='ssh-ed25519 AAAA'
HOST_KEY_DIR=''
RETURNED_HOST_KEY_DIR=''
WORK=''
{functions}
find_registered_deploy_key_id() {{
  n=$(cat {counter!s}); n=$((n + 1)); printf '%s' "$n" > {counter!s}
  [[ "$n" == 1 ]] && printf '42\\n'
  return 0
}}
launchctl() {{ return 0; }}
gh() {{
  if [[ $1 == api && $2 == --method && $3 == DELETE ]]; then
    printf '%s\\n' "$4" >> {delete_log!s}
  fi
  return 0
}}
set +e
false
cleanup
rc=$?
set -e
printf 'rc=%s\\n' "$rc"
[[ "$rc" == 1 ]]
"""
            result = subprocess.run(["bash", "-c", script], text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("rc=1", result.stdout)
            self.assertIn("keys/42", delete_log.read_text())
            self.assertIn("rolled back and verified", result.stderr)

    def test_helper_failures_are_checked_and_host_path_is_predetermined(self):
        source = SCRIPT.read_text()
        self.assertIn('if ! console_exports="$(bash', source)
        self.assertIn('if ! deploy_exports="$(bash', source)
        self.assertIn('if ! host_exports="$(HOST_KEY_DIR=', source)
        self.assertIn('.host-key-paid-', source)
        self.assertNotIn('eval "$(bash', source)


if __name__ == "__main__":
    unittest.main()
