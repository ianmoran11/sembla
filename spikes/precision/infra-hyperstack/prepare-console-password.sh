#!/usr/bin/env bash
set -Eeuo pipefail

cleanup() {
  unset CONSOLE_PASSWORD CONSOLE_PASSWORD_CONFIRM
}
trap cleanup EXIT

if ! command -v openssl >/dev/null 2>&1 || \
  ! printf 'capability-check' | openssl passwd -6 -stdin >/dev/null 2>&1; then
  cat >&2 <<'EOF'
OpenSSL with SHA-512 crypt support is required.
On macOS install Homebrew OpenSSL 3, then put it first on PATH:
  brew install openssl@3
  export PATH="$(brew --prefix openssl@3)/bin:$PATH"
EOF
  exit 1
fi

printf 'Temporary VNC console password: ' >&2
IFS= read -r -s CONSOLE_PASSWORD < /dev/tty
printf '\nRepeat temporary password: ' >&2
IFS= read -r -s CONSOLE_PASSWORD_CONFIRM < /dev/tty
printf '\n' >&2

if [[ -z "$CONSOLE_PASSWORD" || "$CONSOLE_PASSWORD" != "$CONSOLE_PASSWORD_CONFIRM" ]]; then
  echo 'Passwords were empty or did not match; no hash was exported.' >&2
  exit 1
fi

salt="$(openssl rand -hex 8)"
hash="$(printf '%s' "$CONSOLE_PASSWORD" | openssl passwd -6 -salt "$salt" -stdin)"
if [[ ! "$hash" =~ ^\$6\$[./A-Za-z0-9]{1,16}\$[./A-Za-z0-9]{86}$ ]]; then
  echo 'OpenSSL did not produce the required SHA-512 crypt hash.' >&2
  exit 1
fi

# Stdout is deliberately one shell assignment for:
#   eval "$(bash ./prepare-console-password.sh)"
printf "export TF_VAR_console_password_hash='%s'\n" "$hash"
echo 'Console hash prepared; retain the plaintext only in your password manager.' >&2
