#!/usr/bin/env bash
set -Eeuo pipefail

: "${HYPERSTACK_API_KEY:?export HYPERSTACK_API_KEY in this shell; do not put it in tfvars or paste it into chat}"

if [[ "$HYPERSTACK_API_KEY" != "${HYPERSTACK_API_KEY//[[:space:]]/}" ]]; then
  echo "HYPERSTACK_API_KEY must not contain whitespace" >&2
  exit 2
fi

python3 - "$@" <<'PY'
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

# Never allow an environment override to redirect this credentialed request.
BASE = "https://infrahub-api.nexgencloud.com/v1"
KEY = os.environ["HYPERSTACK_API_KEY"]
REGION_ARG = sys.argv[1] if len(sys.argv) > 1 else ""
FULL_RATE = re.compile(r"(?:^|[^A-Za-z0-9])(?:A100|H100|H200|GH200)(?:[^A-Za-z0-9]|$)", re.I)


def get(path):
    request = urllib.request.Request(
        BASE + path,
        headers={
            "api_key": KEY,
            "Accept": "application/json",
            # Hyperstack's edge currently rejects urllib's default user agent
            # even when the same key succeeds with curl.
            "User-Agent": "sembla-precision-discovery/1.0",
        },
        method="GET",
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        # Authenticated bodies are untrusted and may reflect request headers;
        # never print them where the API key could be disclosed.
        if error.code == 401:
            raise SystemExit(
                f"Hyperstack authentication failed for {path} with HTTP 401. "
                "Generate/rotate the key in the console and export it only in this shell."
            )
        if error.code == 403:
            raise SystemExit(
                f"Hyperstack denied {path} with HTTP 403. The key may be valid but "
                "the endpoint, account role, or request user agent is not permitted."
            )
        raise SystemExit(f"Hyperstack API {path} failed with HTTP {error.code}")
    except urllib.error.URLError as error:
        raise SystemExit(f"Hyperstack API {path} could not be reached: {error.reason}")


def rows(payload, *keys):
    if isinstance(payload, list):
        return payload
    if not isinstance(payload, dict):
        return []
    for key in keys + ("data",):
        value = payload.get(key)
        if isinstance(value, list):
            return value
        if isinstance(value, dict):
            for nested in ("data", "items", "results"):
                nested_value = value.get(nested)
                if isinstance(nested_value, list):
                    return nested_value
    return []


def flatten_groups(items, nested_key):
    flattened = []
    for item in items:
        if not isinstance(item, dict):
            continue
        nested = item.get(nested_key)
        if isinstance(nested, list):
            for child in nested:
                if isinstance(child, dict):
                    child = dict(child)
                    child.setdefault("region_name", item.get("region_name"))
                    flattened.append(child)
        else:
            flattened.append(item)
    return flattened


def text(value):
    if isinstance(value, dict):
        return value.get("name") or value.get("model") or json.dumps(value, sort_keys=True)
    return "" if value is None else str(value)


regions_payload = get("/core/regions")
regions = rows(regions_payload, "regions")
region_names = [text(item.get("name")) for item in regions if isinstance(item, dict)]
region = REGION_ARG or ("CANADA-1" if "CANADA-1" in region_names else "")

print("REGIONS")
for item in regions:
    if isinstance(item, dict):
        print(f'- {item.get("name", "?")}: {item.get("description", "") or item.get("country", "")}')
if not regions:
    print("- none returned")

print("\nENVIRONMENTS")
environments = rows(get("/core/environments"), "environments")
for item in environments:
    if isinstance(item, dict):
        print(f'- {item.get("name", "?")}: region={item.get("region", "?")}, id={item.get("id", "?")}')
if not environments:
    print("- none returned")

print("\nSSH KEYPAIRS")
keypairs = rows(get("/core/keypairs"), "keypairs")
for item in keypairs:
    if isinstance(item, dict):
        environment = item.get("environment")
        print(
            f'- {item.get("name", "?")}: environment={text(environment) or "?"}, '
            f'fingerprint={item.get("fingerprint", "?")}, id={item.get("id", "?")}'
        )
if not keypairs:
    print("- none returned")

if not region:
    print("\nNo default region could be selected. Re-run as: bash discover.sh REGION_NAME")
    raise SystemExit(0)

encoded_region = urllib.parse.quote(region, safe="")
print(f"\nFULL-RATE FLAVORS IN {region}")
flavors = flatten_groups(
    rows(get(f"/core/flavors?region={encoded_region}"), "flavors"),
    "flavors",
)
full_rate_flavors = []
for item in flavors:
    if not isinstance(item, dict):
        continue
    gpu = text(item.get("gpu"))
    name = text(item.get("name"))
    if FULL_RATE.search(gpu) or FULL_RATE.search(name):
        full_rate_flavors.append(item)
        print(
            f'- {name}: gpu={gpu}, gpu_count={item.get("gpu_count", "?")}, '
            f'cpu={item.get("cpu", "?")}, ram={item.get("ram", "?")}, '
            f'stock_available={item.get("stock_available", "?")}'
        )
if not full_rate_flavors:
    print("- none currently returned")

print(f"\nFULL-RATE STOCK IN {region}")
stocks = rows(get("/core/stocks"), "stocks")
stock_found = False
for stock in stocks:
    if not isinstance(stock, dict) or stock.get("region") != region:
        continue
    for model in stock.get("models", []) or []:
        if isinstance(model, dict) and FULL_RATE.search(text(model.get("model"))):
            stock_found = True
            config = model.get("configurations", {}) or {}
            print(
                f'- {model.get("model", "?")}: available={model.get("available", "?")}, '
                f'1x={config.get("1x", config.get("n1x", "?"))}, '
                f'stocktype={stock.get("stock-type", stock.get("stocktype", "?"))}'
            )
if not stock_found:
    print("- none currently returned")

print(f"\nUBUNTU CUDA IMAGES IN {region}")
query = urllib.parse.urlencode({"region": region, "include_public": "true", "search": "CUDA"})
images = flatten_groups(rows(get(f"/core/images?{query}"), "images"), "images")
image_found = False
for item in images:
    if not isinstance(item, dict):
        continue
    name = text(item.get("name"))
    if re.search(r"ubuntu", name, re.I) and re.search(r"cuda", name, re.I):
        image_found = True
        print(
            f'- {name}: version={item.get("version", "?")}, type={item.get("type", "?")}, '
            f'region={item.get("region_name", region)}, public={item.get("is_public", "?")}'
        )
if not image_found:
    print("- none currently returned")

print("\nRELEVANT PRICEBOOK ENTRIES")
pricebook = rows(get("/pricebook"), "pricebook", "prices")
price_found = False
candidate_flavor_names = {text(item.get("name")) for item in full_rate_flavors}
for item in pricebook:
    if not isinstance(item, dict):
        continue
    name = text(item.get("name"))
    if name in candidate_flavor_names or FULL_RATE.search(name) or re.search(r"floating|public.?ip", name, re.I):
        price_found = True
        print(
            f'- {name}: value={item.get("value", item.get("actual_price", "?"))}, '
            f'original_value={item.get("original_value", item.get("price", "?"))}, '
            f'discount_applied={item.get("discount_applied", "?")}'
        )
if not price_found:
    print("- no model-named entries returned; review the console price shown for the exact flavor")

print("\nDiscovery was read-only. No VM, IP, disk, or firewall rule was created.")
print(f"Selected discovery region: {region}")
PY
