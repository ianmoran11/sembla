#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

scales="${SCALES:-10000,100000,1000000}"
seed="${SEED:-9009}"
ticks="${TICKS:-24}"
out_dir="${OUT_DIR:-$repo_root/bench-demographic-output}"
areas="${AREAS:-4}"
present_fraction="${PRESENT_FRACTION:-0.8}"
streams="${STREAMS:-birth:600,overseas:250,internal:150}"
backend="${BACKEND:-cpu}"
machine_class="${MACHINE_CLASS:-local moderate-memory CPU machine}"
sembla="${SEMBLA_BIN:-}"

usage() {
  cat <<'EOF'
usage: scripts/bench-demographic.sh [--scales N,N,...] [--seed N] [--ticks N]
       [--out DIR] [--backend cpu|cuda] [--machine-class TEXT]
       [--sembla PATH]

Environment equivalents: SCALES, SEED, TICKS, OUT_DIR, BACKEND,
MACHINE_CLASS, SEMBLA_BIN, AREAS, PRESENT_FRACTION, STREAMS.
The default CPU scales are 10000,100000,1000000. Scale 50000000 is accepted
but must only be run on the documented hardware. CUDA mode benchmarks the
no-grouped model because grouped observations are CPU-only (DECISIONS §K6).
EOF
}

while (($#)); do
  case "$1" in
    --scales) scales="${2:?missing --scales value}"; shift 2 ;;
    --seed) seed="${2:?missing --seed value}"; shift 2 ;;
    --ticks) ticks="${2:?missing --ticks value}"; shift 2 ;;
    --out) out_dir="${2:?missing --out value}"; shift 2 ;;
    --backend) backend="${2:?missing --backend value}"; shift 2 ;;
    --machine-class) machine_class="${2:?missing --machine-class value}"; shift 2 ;;
    --sembla) sembla="${2:?missing --sembla value}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument '$1'" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ "$backend" != cpu && "$backend" != cuda ]]; then
  echo "error: --backend must be cpu or cuda" >&2
  exit 2
fi
if [[ ! "$seed" =~ ^[0-9]+$ || ! "$ticks" =~ ^[1-9][0-9]*$ ]]; then
  echo "error: seed must be nonnegative and ticks must be positive" >&2
  exit 2
fi
IFS=',' read -r -a scale_list <<< "$scales"
for scale in "${scale_list[@]}"; do
  if [[ ! "$scale" =~ ^[1-9][0-9]*$ ]]; then
    echo "error: invalid positive scale '$scale'" >&2
    exit 2
  fi
done

if [[ -z "$sembla" ]]; then
  cargo build --quiet --release --locked -p sembla-cli
  target_dir="${CARGO_TARGET_DIR:-target}"
  [[ "$target_dir" = /* ]] || target_dir="$repo_root/$target_dir"
  sembla="$target_dir/release/sembla"
fi
if [[ ! -x "$sembla" ]]; then
  echo "error: Sembla executable not found at '$sembla'" >&2
  exit 2
fi
if [[ ! -x /usr/bin/time ]]; then
  echo "error: /usr/bin/time is required" >&2
  exit 2
fi

rm -rf "$out_dir/work"
mkdir -p "$out_dir/work"
rm -f "$out_dir/bench-results.json" "$out_dir/bench-results.md"
metrics="$out_dir/work/results.jsonl"
: > "$metrics"

os_name="$(uname -s)"
os_release="$(uname -r)"
arch="$(uname -m)"
if [[ "$os_name" == Darwin ]]; then
  cpu="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)"
  ram_bytes="$(sysctl -n hw.memsize)"
  time_mode=darwin
else
  cpu="$(awk -F: '/model name/{sub(/^[ \t]+/,"",$2); print $2; exit}' /proc/cpuinfo 2>/dev/null || true)"
  [[ -n "$cpu" ]] || cpu=unknown
  ram_kib="$(awk '/MemTotal:/{print $2; exit}' /proc/meminfo 2>/dev/null || echo 0)"
  ram_bytes="$((ram_kib * 1024))"
  time_mode=linux
fi

MEASURE_SECONDS=
MEASURE_RSS_BYTES=
measure() {
  local label="$1"; shift
  local timing="$out_dir/work/$label.time"
  local stdout="$out_dir/work/$label.stdout"
  local stderr="$out_dir/work/$label.stderr"
  if [[ "$time_mode" == darwin ]]; then
    /usr/bin/time -l -o "$timing" "$@" >"$stdout" 2>"$stderr"
  else
    /usr/bin/time -v -o "$timing" "$@" >"$stdout" 2>"$stderr"
  fi
  read -r MEASURE_SECONDS MEASURE_RSS_BYTES < <(
    python3 - "$timing" "$time_mode" <<'PY'
import re, sys
text = open(sys.argv[1], encoding="utf-8").read()
mode = sys.argv[2]
if mode == "darwin":
    elapsed = re.search(r"(?m)^\s*([0-9.]+)\s+real\b", text)
    rss = re.search(r"(?m)^\s*([0-9]+)\s+maximum resident set size\b", text)
    rss_bytes = int(rss.group(1)) if rss else 0
else:
    # GNU time prints: "Elapsed (wall clock) time (h:mm:ss or m:ss): 0:01.23".
    # The label itself contains colons, so anchoring on the first colon captures
    # "mm:ss" out of the label rather than the value. Match the whole line and
    # take its final whitespace-separated token, which is the value in every
    # documented GNU time format.
    line = re.search(r"(?m)^.*Elapsed \(wall clock\) time.*$", text)
    elapsed = None
    if line:
        tokens = line.group(0).split()
        if tokens:
            # Group 1 must be the whole value: the shared code below reads
            # elapsed.group(1), so inner groups are non-capturing.
            elapsed = re.fullmatch(r"([0-9]+(?::[0-9]+)*(?:\.[0-9]+)?)", tokens[-1])
    rss = re.search(r"Maximum resident set size \(kbytes\):\s*([0-9]+)", text)
    rss_bytes = int(rss.group(1)) * 1024 if rss else 0
if not elapsed:
    raise SystemExit("could not parse elapsed time from /usr/bin/time output")
parts = elapsed.group(1).split(":")
seconds = 0.0
for part in parts:
    seconds = seconds * 60.0 + float(part)
print(f"{seconds:.9f} {rss_bytes}")
PY
  )
}

resize_variants() {
  local scale="$1" full_companion="$2" dir="$3"
  python3 - "$scale" "$full_companion" "$dir" <<'PY'
import json, pathlib, sys
scale = int(sys.argv[1])
full_companion = pathlib.Path(sys.argv[2])
out = pathlib.Path(sys.argv[3])
root = pathlib.Path.cwd()
templates = {
    "full": full_companion,
    "no-ageing": root / "fixtures/demographic/benchmark/demographic_slots.no-ageing.json",
    "no-grouped": root / "fixtures/demographic/benchmark/demographic_slots.no-grouped.json",
}
for name, path in templates.items():
    model = json.loads(path.read_text())
    for box in model["boxes"]:
        for table in box["tables"]:
            if table["name"] == "area":
                continue
            if table["name"] in {"person_slot", "slot_resource"}:
                table["size_hint"] = scale
    (out / f"{name}.json").write_text(json.dumps(model, separators=(",", ":")) + "\n")
load = json.loads((out / "full.json").read_text())
load["summaries"] = []
(out / "load-only.json").write_text(json.dumps(load, separators=(",", ":")) + "\n")
PY
}

for scale in "${scale_list[@]}"; do
  scale_dir="$out_dir/work/$scale"
  mkdir -p "$scale_dir"
  state="$scale_dir/initial.state"

  measure "${scale}-synth" "$sembla" synth-state \
    --model fixtures/demographic/benchmark/demographic_slots.full.json \
    --slots "$scale" --areas "$areas" --present-fraction "$present_fraction" \
    --streams "$streams" --seed "$seed" --out "$state"
  synth_seconds="$MEASURE_SECONDS"
  synth_rss="$MEASURE_RSS_BYTES"
  state_bytes="$(wc -c < "$state" | tr -d ' ')"
  companion="$state.model.json"
  resize_variants "$scale" "$companion" "$scale_dir"

  common=(--seed "$seed" --population "$state")
  measure "${scale}-load" "$sembla" run "$scale_dir/load-only.json" \
    "${common[@]}" --backend cpu --ticks 0 --enable grouped-observations
  load_seconds="$MEASURE_SECONDS"
  load_rss="$MEASURE_RSS_BYTES"

  full_seconds=null
  full_rss=0
  no_ageing_seconds=null
  no_ageing_rss=0
  no_grouped_seconds=null
  no_grouped_rss=0
  ageing_share=null
  grouped_share=null
  ticks_per_second=null

  if [[ "$backend" == cpu ]]; then
    measure "${scale}-full" "$sembla" run "$scale_dir/full.json" \
      "${common[@]}" --backend cpu --ticks "$ticks" --out "$scale_dir/full.csv" \
      --enable grouped-observations
    full_seconds="$MEASURE_SECONDS"; full_rss="$MEASURE_RSS_BYTES"

    measure "${scale}-no-ageing" "$sembla" run "$scale_dir/no-ageing.json" \
      "${common[@]}" --backend cpu --ticks "$ticks" --out "$scale_dir/no-ageing.csv" \
      --enable grouped-observations
    no_ageing_seconds="$MEASURE_SECONDS"; no_ageing_rss="$MEASURE_RSS_BYTES"

    measure "${scale}-no-grouped" "$sembla" run "$scale_dir/no-grouped.json" \
      "${common[@]}" --backend cpu --ticks "$ticks" --out "$scale_dir/no-grouped.csv"
    no_grouped_seconds="$MEASURE_SECONDS"; no_grouped_rss="$MEASURE_RSS_BYTES"

    read -r ticks_per_second ageing_share grouped_share < <(
      python3 - "$ticks" "$full_seconds" "$no_ageing_seconds" "$no_grouped_seconds" <<'PY'
import sys
ticks, full, no_age, no_group = map(float, sys.argv[1:])
if full <= 0.0:
    print("null null null")
else:
    print(f"{ticks/full:.9f} {(full-no_age)/full:.9f} {(full-no_group)/full:.9f}")
PY
    )
  else
    measure "${scale}-no-grouped" "$sembla" run "$scale_dir/no-grouped.json" \
      "${common[@]}" --backend cuda --ticks "$ticks" --out "$scale_dir/no-grouped.csv"
    no_grouped_seconds="$MEASURE_SECONDS"; no_grouped_rss="$MEASURE_RSS_BYTES"
    ticks_per_second="$(python3 -c 'import sys; t=float(sys.argv[1]); s=float(sys.argv[2]); print("null" if s <= 0 else f"{t/s:.9f}")' "$ticks" "$no_grouped_seconds")"
  fi

  exported="$scale_dir/exported.state"
  if [[ "$backend" == cpu ]]; then
    measure "${scale}-export" "$sembla" run "$scale_dir/load-only.json" \
      "${common[@]}" --backend cpu --ticks 0 --enable grouped-observations \
      --export-state "$exported"
  else
    measure "${scale}-export" "$sembla" run "$scale_dir/no-grouped.json" \
      "${common[@]}" --backend cuda --ticks 0 --export-state "$exported"
  fi
  export_seconds="$MEASURE_SECONDS"
  export_rss="$MEASURE_RSS_BYTES"
  export_bytes="$(wc -c < "$exported" | tr -d ' ')"

  python3 - "$metrics" "$scale" "$synth_seconds" "$synth_rss" "$state_bytes" \
    "$load_seconds" "$load_rss" "$full_seconds" "$full_rss" \
    "$no_ageing_seconds" "$no_ageing_rss" "$no_grouped_seconds" "$no_grouped_rss" \
    "$export_seconds" "$export_rss" "$export_bytes" "$ticks_per_second" \
    "$ageing_share" "$grouped_share" <<'PY'
import json, sys
p = sys.argv[1]
def number(v): return None if v == "null" else float(v)
row = {
  "scale": int(sys.argv[2]),
  "synth_state": {"wall_seconds": float(sys.argv[3]), "peak_rss_bytes": int(sys.argv[4]), "artifact_bytes": int(sys.argv[5])},
  "load": {"wall_seconds": float(sys.argv[6]), "peak_rss_bytes": int(sys.argv[7]), "ticks": 0, "summary_free_working_model": True},
  "full": {"wall_seconds": number(sys.argv[8]), "peak_rss_bytes": int(sys.argv[9])},
  "no_ageing": {"wall_seconds": number(sys.argv[10]), "peak_rss_bytes": int(sys.argv[11])},
  "no_grouped": {"wall_seconds": number(sys.argv[12]), "peak_rss_bytes": int(sys.argv[13])},
  "export_state": {"wall_seconds": float(sys.argv[14]), "peak_rss_bytes": int(sys.argv[15]), "artifact_bytes": int(sys.argv[16])},
  "derived": {"ticks_per_second": number(sys.argv[17]), "ageing_cost_share": number(sys.argv[18]), "grouped_observation_cost_share": number(sys.argv[19])},
}
with open(p, "a", encoding="utf-8") as f: f.write(json.dumps(row, separators=(",", ":")) + "\n")
PY
  echo "benchmarked scale=$scale backend=$backend" >&2
done

python3 - "$metrics" "$out_dir/bench-results.json" "$out_dir/bench-results.md" \
  "$os_name" "$os_release" "$arch" "$cpu" "$ram_bytes" "$machine_class" \
  "$backend" "$seed" "$ticks" "$areas" "$present_fraction" "$streams" <<'PY'
import datetime, json, sys
metrics, json_out, md_out = sys.argv[1:4]
os_name, os_release, arch, cpu, ram, machine_class = sys.argv[4:10]
backend, seed, ticks, areas, fraction, streams = sys.argv[10:16]
rows = [json.loads(line) for line in open(metrics, encoding="utf-8") if line.strip()]
doc = {
  "schema_version": "sembla.demographic-benchmark/v1",
  "evidence_date_utc": datetime.datetime.now(datetime.timezone.utc).date().isoformat(),
  "machine": {"class": machine_class, "os": os_name, "os_release": os_release, "arch": arch, "cpu": cpu, "ram_bytes": int(ram)},
  "configuration": {"backend": backend, "seed": int(seed), "ticks": int(ticks), "areas": int(areas), "present_fraction": float(fraction), "streams": streams},
  "results": rows,
}
with open(json_out, "w", encoding="utf-8") as f: json.dump(doc, f, indent=2, sort_keys=True); f.write("\n")
def fmt(v, digits=3): return "pending" if v is None else f"{v:.{digits}f}"
lines = [
  "# Demographic benchmark results",
  "",
  f"Machine class: **{machine_class}**; OS `{os_name} {os_release}`; architecture `{arch}`; CPU `{cpu}`; RAM `{int(ram)/(1024**3):.1f} GiB`. No hostname or workspace path is recorded.",
  "",
  f"Backend: `{backend}`; seed: `{seed}`; ticks: `{ticks}`; present fraction: `{fraction}`; streams: `{streams}`.",
  "",
  "| Slots | State MiB | Synth s | Load s | Full s | No-ageing s | No-grouped s | ticks/s | Ageing share | Grouped share | Peak RSS MiB | Export s |",
  "|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
]
for r in rows:
  full = r["full"]["wall_seconds"]
  peak = max(r[x]["peak_rss_bytes"] for x in ["synth_state","load","full","no_ageing","no_grouped","export_state"])
  d = r["derived"]
  lines.append("| {scale:,} | {mib:.2f} | {synth:.3f} | {load:.3f} | {full} | {age} | {group} | {tps} | {ashare} | {gshare} | {rss:.1f} | {export:.3f} |".format(
    scale=r["scale"], mib=r["synth_state"]["artifact_bytes"]/(1024**2), synth=r["synth_state"]["wall_seconds"], load=r["load"]["wall_seconds"],
    full=fmt(full), age=fmt(r["no_ageing"]["wall_seconds"]), group=fmt(r["no_grouped"]["wall_seconds"]), tps=fmt(d["ticks_per_second"]),
    ashare=fmt(d["ageing_cost_share"]), gshare=fmt(d["grouped_observation_cost_share"]), rss=peak/(1024**2), export=r["export_state"]["wall_seconds"]))
lines += ["", "Cost shares are `(full − variant) / full` wall time. Negative values are retained rather than hidden because these are single local measurements.", ""]
open(md_out, "w", encoding="utf-8").write("\n".join(lines))
PY

rm -rf "$out_dir/work"
echo "wrote $out_dir/bench-results.json and $out_dir/bench-results.md" >&2
