#!/usr/bin/env python3
"""Print a credential-free, approval-focused summary of one saved Hyperstack plan."""

from __future__ import annotations

import hashlib
import ipaddress
import json
import re
import stat
import subprocess
import sys
from pathlib import Path

EXPECTED = {
    "hyperstack_core_virtual_machine.gpu[0]",
    "hyperstack_core_virtual_machine_sg_rule.ssh[0]",
}


def fail(message: str) -> None:
    raise SystemExit(f"paid-plan review failed: {message}")


def main() -> None:
    if len(sys.argv) != 2:
        fail("usage: review-paid-plan.py PLAN_FILE")
    plan = Path(sys.argv[1])
    if not plan.is_file() or plan.stat().st_size == 0:
        fail(f"missing or empty plan: {plan}")
    plan_mode = stat.S_IMODE(plan.stat().st_mode)
    if plan_mode & 0o077:
        fail(
            f"saved plan permissions {plan_mode:#o} expose sensitive user_data; "
            "set umask 077 and re-plan"
        )

    shown = subprocess.run(
        ["terraform", "show", "-json", str(plan)],
        check=True,
        capture_output=True,
        text=True,
    )
    document = json.loads(shown.stdout)
    resources = [
        change
        for change in document.get("resource_changes", [])
        if change.get("mode") == "managed"
        and change.get("change", {}).get("actions") != ["no-op"]
    ]
    addresses = {change.get("address") for change in resources}
    if addresses != EXPECTED:
        fail(f"resource actions differ from the exact two-resource allowlist: {sorted(addresses)}")
    if any(change.get("change", {}).get("actions") != ["create"] for change in resources):
        fail("both allowed resources must have create-only actions")

    by_address = {change["address"]: change["change"] for change in resources}
    vm_change = by_address["hyperstack_core_virtual_machine.gpu[0]"]
    if vm_change.get("after_sensitive", {}).get("user_data") is not True:
        fail("VM user_data is not marked sensitive in the saved plan")
    vm = vm_change["after"]
    rule = by_address["hyperstack_core_virtual_machine_sg_rule.ssh[0]"]["after"]
    profile = (
        document.get("planned_values", {})
        .get("outputs", {})
        .get("selected_profile", {})
        .get("value")
    )
    if not isinstance(profile, dict):
        fail("selected_profile output is unavailable")

    cidr = ipaddress.ip_network(str(profile.get("ssh_cidr", "")), strict=True)
    if cidr.prefixlen != 32 or not cidr.is_global:
        fail(f"SSH CIDR is not one global IPv4 /32: {cidr}")
    if rule.get("remote_ip_prefix") != str(cidr):
        fail("security-group rule CIDR differs from selected_profile")
    if (
        rule.get("direction") != "ingress"
        or rule.get("ethertype") != "IPv4"
        or rule.get("protocol") != "tcp"
        or rule.get("port_range_min") != 22
        or rule.get("port_range_max") != 22
    ):
        fail("security-group rule is not exactly inbound TCP/22")
    if (
        vm.get("environment_name") != profile.get("environment")
        or vm.get("flavor_name") != profile.get("flavor")
        or vm.get("image_name") != profile.get("image")
        or vm.get("assign_floating_ip") is not True
        or vm.get("create_bootable_volume") is not False
        or vm.get("enable_port_randomization") is not False
    ):
        fail("VM fields differ from the selected profile or safety settings")
    if profile.get("expected_gpu") not in {"A100", "H100", "H200", "GH200"}:
        fail("selected GPU is not in the full-rate allowlist")
    if profile.get("expected_gpu_count") != 1:
        fail("selected profile is not exactly one GPU")
    price = profile.get("expected_hourly_price_usd")
    cap = profile.get("max_hourly_price_usd")
    if not isinstance(price, (int, float)) or not isinstance(cap, (int, float)) or not 0 < price <= cap:
        fail("reviewed hourly price is invalid or exceeds the cap")
    repository_ref = str(profile.get("repository_ref", ""))
    if not re.fullmatch(r"[0-9a-f]{40}", repository_ref) or set(repository_ref) == {"0"}:
        fail("repository_ref is not a nonzero lowercase 40-hex commit")
    user_data = vm.get("user_data")
    if not isinstance(user_data, str) or not user_data:
        fail("VM user_data is missing")

    summary = {
        "saved_plan": str(plan),
        "saved_plan_mode": f"{plan_mode:#o}",
        "saved_plan_sha256": hashlib.sha256(plan.read_bytes()).hexdigest(),
        "actions": [
            {
                "address": change["address"],
                "actions": change["change"]["actions"],
            }
            for change in sorted(resources, key=lambda item: item["address"])
        ],
        "selected_profile": profile,
        "vm": {
            "name": vm.get("name"),
            "environment_name": vm.get("environment_name"),
            "flavor_name": vm.get("flavor_name"),
            "image_name": vm.get("image_name"),
            "key_name": vm.get("key_name"),
            "assign_floating_ip": vm.get("assign_floating_ip"),
            "create_bootable_volume": vm.get("create_bootable_volume"),
            "enable_port_randomization": vm.get("enable_port_randomization"),
            "user_data_sensitive": True,
        },
        "ssh_rule": {
            "direction": rule.get("direction"),
            "ethertype": rule.get("ethertype"),
            "protocol": rule.get("protocol"),
            "port_range_min": rule.get("port_range_min"),
            "port_range_max": rule.get("port_range_max"),
            "remote_ip_prefix": rule.get("remote_ip_prefix"),
        },
    }
    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
