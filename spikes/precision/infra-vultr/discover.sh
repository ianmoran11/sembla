#!/usr/bin/env bash
set -euo pipefail

: "${VULTR_API_KEY:?Revoke the key exposed in chat, create a replacement, and export VULTR_API_KEY locally}"

python3 - <<'PY'
import json
import os
import re
import sys
import urllib.error
import urllib.request

BASE = "https://api.vultr.com/v2"
RAW_TOKEN = os.environ["VULTR_API_KEY"]
TOKEN = RAW_TOKEN.strip()
if TOKEN != RAW_TOKEN or len(TOKEN) < 20:
    print(
        "VULTR_API_KEY contains surrounding whitespace or is unexpectedly short; "
        "re-export the replacement key without printing it.",
        file=sys.stderr,
    )
    raise SystemExit(2)
FULL_RATE = re.compile(r"(?:^|[^A-Z0-9])(?:A100|H100|H200|GH200|B200)(?:$|[^A-Z0-9])", re.I)


def get(path):
    req = urllib.request.Request(
        BASE + path,
        headers={
            "Authorization": f"Bearer {TOKEN}",
            "User-Agent": "sembla-precision-discovery/1",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 401:
            print(
                "Vultr rejected the API credential (HTTP 401). Revoke any key "
                "previously pasted into chat, create a replacement, export it "
                "as VULTR_API_KEY, and allow your current public IPv4 under "
                "Vultr Account > API > Access Control. Do not paste the key here.",
                file=sys.stderr,
            )
        else:
            print(f"Vultr API request failed for {path}: HTTP {error.code}", file=sys.stderr)
        raise SystemExit(2) from None


# Authenticate before catalog discovery so an invalid or IP-restricted key
# produces one actionable error instead of a Python traceback.
get("/account")
regions = get("/regions?per_page=500").get("regions", [])
region_names = {r["id"]: f'{r.get("city", "?")}, {r.get("country", "?")}' for r in regions}
cloud = get("/plans?per_page=500").get("plans", [])
bare_payload = get("/plans-metal?per_page=500")
bare = bare_payload.get("plans_metal", bare_payload.get("plans", []))
ssh_keys = get("/ssh-keys?per_page=500").get("ssh_keys", [])
os_images = get("/os?per_page=500").get("os", [])


def locations_text(ids):
    if not ids:
        return "none currently advertised"
    return ", ".join(f"{i} ({region_names.get(i, '?')})" for i in ids)


print("FULL-RATE CLOUD GPU CANDIDATES")
found = False
for plan in cloud:
    gpu = plan.get("gpu_type", "")
    if FULL_RATE.search(gpu):
        found = True
        monthly = float(plan.get("monthly_cost", 0))
        print(
            f'- {plan["id"]}: {gpu}, VRAM={plan.get("gpu_vram", "?")} MB, '
            f'${monthly:.2f}/month (~${monthly / 730:.4f}/hour), '
            f'locations={locations_text(plan.get("locations", []))}'
        )
if not found:
    print("- none visible to this account")

print("\nFULL-RATE BARE-METAL CANDIDATES")
found = False
for plan in bare:
    if FULL_RATE.search(plan.get("id", "")):
        found = True
        monthly = float(plan.get("monthly_cost", 0))
        print(
            f'- {plan["id"]}: GPUs={plan.get("gpu_count", "?")}, '
            f'${monthly:.2f}/month (~${monthly / 730:.4f}/hour), '
            f'locations={locations_text(plan.get("locations", []))}'
        )
if not found:
    print("- none visible to this account")

print("\nSSH KEYS")
if ssh_keys:
    for key in ssh_keys:
        print(f'- {key.get("name", "unnamed")}: {key.get("id", "missing-id")}')
else:
    print("- none; add an SSH key in the Vultr console before any apply")

print("\nUBUNTU IMAGES")
for image in os_images:
    if image.get("family") == "ubuntu":
        print(f'- {image.get("name")}: os_id={image.get("id")}, arch={image.get("arch")}')

print("\nDiscovery is read-only. A listed location is not a capacity guarantee.")
PY
