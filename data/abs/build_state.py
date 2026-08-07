#!/usr/bin/env python3
"""Build deterministic 30 June 2010 Australian population state artifacts.

The plan is compressed into homogeneous slot groups.  It can therefore prove
all allocation arithmetic without materialising millions of Python row objects;
column factories expand those groups only while the binary artifact is streamed.
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
import json
import pathlib
import re
import sys
from typing import Iterable

import scaling
import state_artifact


HERE = pathlib.Path(__file__).resolve().parent
EXTRACTS = HERE / "extracts"
FIRST_RUN_YEAR = 2010
LAST_RUN_YEAR = 2024
HEADROOM_NUMERATOR = 1
HEADROOM_DENOMINATOR = 10
SCALES = {"full": 1, "tenth": 10, "hundredth": 100}
ARTIFACT_EVIDENCE = {
    "full": {
        "path": "data/abs/generated/australian_population_2010_full.state",
        "bytes": 1_691_804_647,
        "sha256": "55429a20f03d2a0f63d1490ca13e0191579451043c99b573bed037fb398461ae",
        "state_hash": "ef3c93722199cf96b4d7839d79561de1f4c0e944924c4a9d490e64ba6a92c083",
    },
    "tenth": {
        "path": "data/abs/generated/australian_population_2010_tenth.state",
        "bytes": 169_181_189,
        "sha256": "b6f7e827df3da3d247f4194194c0ed3770e16677bc71b739cd617c19cbd96ee2",
        "state_hash": "926fc80a0330c764cac8c4a69c09dee6b4d088653dc9bf830c9e068a2b87c02a",
    },
    "hundredth": {
        "path": "fixtures/state/australian_population_2010_hundredth.state",
        "bytes": 16_918_851,
        "sha256": "1d3f85db8fd93c66118df15622c70eac4fd6dfc1adcc72c9142b5949146eff5f",
        "state_hash": "c7db0d7324aecd9a50a3d297e604f71da8677058c20ae9b42f8fd7524a136df4",
    },
}
STATE_CODES = ("act", "nsw", "nt", "qld", "sa", "tas", "vic", "wa")
SEXES = ("female", "male")
AGE_BANDS = (
    "0-4", "5-9", "10-14", "15-19", "20-24", "25-29", "30-34",
    "35-39", "40-44", "45-49", "50-54", "55-59", "60-64", "65+",
)

PERSON_TABLE = ("demographic", "person_slot")
RESOURCE_TABLE = ("demographic", "slot_resource")
PERSON_COLUMNS = (
    "occupancy", "event", "sex", "age_months", "event_age_months",
    "generation", "entry_stream", "entry_age_months", "area", "prev_area",
    "slot_resource",
)


@dataclass(frozen=True)
class SourceTotals:
    erp: dict[tuple[str, str, int], int]
    births_state: dict[str, int]
    births_sex: dict[str, int]
    nom_detail: dict[tuple[str, str, str], int]
    births_required: int
    overseas_required: int
    births_headroom: int
    overseas_headroom: int
    national_birth_residual: int
    nom_detail_residual: int


@dataclass(frozen=True)
class SlotGroup:
    """A canonical run of rows sharing every value except a possible age spread."""

    kind: str
    state: str
    sex: str
    source_age: int | str
    count: int
    occupancy: str
    entry_stream: str
    age_base_months: int
    age_spread_months: int
    entry_age_months: int

    def age_months(self, index: int) -> int:
        if not 0 <= index < self.count:
            raise IndexError(index)
        if not self.age_spread_months:
            return self.age_base_months
        return self.age_base_months + index * self.age_spread_months // self.count


@dataclass(frozen=True)
class StatePlan:
    scale: str
    divisor: int
    groups: tuple[SlotGroup, ...]
    present_counts: dict[tuple[str, str, int], int]
    present_errors: dict[tuple[str, str, int], int]
    birth_counts: dict[tuple[str, str], int]
    overseas_counts: dict[tuple[str, str, str], int]
    present_slots: int
    birth_slots: int
    overseas_slots: int

    @property
    def total_slots(self) -> int:
        return self.present_slots + self.birth_slots + self.overseas_slots


def _rows(name: str) -> list[dict[str, str]]:
    path = EXTRACTS / name
    with path.open(newline="", encoding="utf-8") as source:
        return list(csv.DictReader(source))


def _require_years(label: str, years: set[int]) -> None:
    expected = set(range(FIRST_RUN_YEAR, LAST_RUN_YEAR + 1))
    if years != expected:
        raise ValueError(
            f"{label} years changed: missing={sorted(expected - years)!r}, "
            f"extra={sorted(years - expected)!r}"
        )


def _headroom(required: int) -> int:
    numerator = required * HEADROOM_NUMERATOR
    return (numerator + HEADROOM_DENOMINATOR - 1) // HEADROOM_DENOMINATOR


def load_source_totals() -> SourceTotals:
    """Load and validate every canonical input used by the state builder."""
    erp_rows = _rows("erp_state_age_sex.csv")
    erp = {
        (row["state"], row["sex"], int(row["age"])): int(row["persons"])
        for row in erp_rows if int(row["year"]) == FIRST_RUN_YEAR
    }
    expected_erp = {
        (state, sex, age)
        for state in STATE_CODES for sex in SEXES for age in range(101)
    }
    if set(erp) != expected_erp:
        raise ValueError("2010 ERP does not contain the fixed 8 x 2 x 101 grid")

    birth_rows = _rows("births_state.csv")
    _require_years("births_state", {int(row["year"]) for row in birth_rows})
    births_state = {state: 0 for state in STATE_CODES}
    for row in birth_rows:
        state = row["state"]
        if state not in births_state:
            raise ValueError(f"births_state has unknown state {state!r}")
        births_state[state] += int(row["births"])

    birth_sex_rows = _rows("births_sex.csv")
    _require_years("births_sex", {int(row["year"]) for row in birth_sex_rows})
    births_sex = {sex: 0 for sex in SEXES}
    for row in birth_sex_rows:
        sex = row["sex"]
        if sex not in births_sex:
            raise ValueError(f"births_sex has unknown sex {sex!r}")
        births_sex[sex] += int(row["births"])

    margin_rows = _rows("overseas_margins.csv")
    _require_years(
        "overseas_margins", {int(row["run_year"]) for row in margin_rows}
    )
    overseas_required = sum(int(row["arrivals"]) for row in margin_rows)

    nom_rows = _rows("nom_state_age_sex.csv")
    _require_years("nom_state_age_sex", {int(row["year"]) for row in nom_rows})
    nom_detail = {
        (state, sex, age_band): 0
        for state in STATE_CODES for sex in SEXES for age_band in AGE_BANDS
    }
    for row in nom_rows:
        key = (row["state"], row["sex"], row["age_band"])
        if key not in nom_detail:
            raise ValueError(f"nom_state_age_sex has unknown cell {key!r}")
        nom_detail[key] += int(row["arrivals"])

    births_required = sum(births_state.values())
    national_births = sum(births_sex.values())
    national_birth_residual = national_births - births_required
    if not 0 <= national_birth_residual <= 15 * 50:
        raise ValueError(
            f"national/eight-state birth residual changed to {national_birth_residual}"
        )
    nom_detail_residual = sum(nom_detail.values()) - overseas_required
    if abs(nom_detail_residual) > 15 * len(STATE_CODES) * 50:
        raise ValueError(
            f"detailed/margin overseas residual changed to {nom_detail_residual}"
        )

    return SourceTotals(
        erp=erp,
        births_state=births_state,
        births_sex=births_sex,
        nom_detail=nom_detail,
        births_required=births_required,
        overseas_required=overseas_required,
        births_headroom=_headroom(births_required),
        overseas_headroom=_headroom(overseas_required),
        national_birth_residual=national_birth_residual,
        nom_detail_residual=nom_detail_residual,
    )


def _age_band_lower(age_band: str) -> int:
    if age_band == "65+":
        return 65
    try:
        return int(age_band.split("-", 1)[0])
    except (ValueError, IndexError) as error:
        raise ValueError(f"invalid age band {age_band!r}") from error


def build_plan(scale: str, sources: SourceTotals | None = None) -> StatePlan:
    """Construct one deterministic compressed state plan."""
    if scale not in SCALES:
        raise ValueError(f"unknown scale {scale!r}; expected one of {sorted(SCALES)!r}")
    divisor = SCALES[scale]
    sources = sources or load_source_totals()

    present = scaling.scale_with_margins(
        sources.erp,
        divisor,
        row_of=lambda key: key[0],
        column_of=lambda key: key[2],
    )

    birth_total_full = sources.births_required + sources.births_headroom
    birth_target = scaling.rounded_total(birth_total_full, divisor)
    birth_weights = {
        (state, sex): sources.births_state[state] * sources.births_sex[sex]
        for state in STATE_CODES for sex in SEXES
    }
    births = scaling.apportion_with_margins(
        birth_weights,
        birth_target,
        row_of=lambda key: key[0],
        column_of=lambda key: key[1],
    )

    overseas_total_full = sources.overseas_required + sources.overseas_headroom
    overseas_target = scaling.rounded_total(overseas_total_full, divisor)
    overseas = scaling.apportion_with_margins(
        sources.nom_detail,
        overseas_target,
        row_of=lambda key: (key[0], key[1]),
        column_of=lambda key: key[2],
    )

    groups: list[SlotGroup] = []
    for key in sorted(present.counts):
        state, sex, age = key
        count = present.counts[key]
        if count:
            groups.append(SlotGroup(
                kind="present", state=state, sex=sex, source_age=age,
                count=count, occupancy="present", entry_stream="retired_slot",
                age_base_months=age * 12, age_spread_months=12,
                entry_age_months=0,
            ))
    for key in sorted(births.counts):
        state, sex = key
        count = births.counts[key]
        if count:
            groups.append(SlotGroup(
                kind="birth", state=state, sex=sex, source_age=0,
                count=count, occupancy="vacant", entry_stream="birth_slot",
                age_base_months=0, age_spread_months=0, entry_age_months=0,
            ))
    age_order = {value: index for index, value in enumerate(AGE_BANDS)}
    for key in sorted(
        overseas.counts,
        key=lambda value: (value[0], value[1], age_order[value[2]]),
    ):
        state, sex, age_band = key
        count = overseas.counts[key]
        if count:
            entry_age = _age_band_lower(age_band) * 12
            groups.append(SlotGroup(
                kind="overseas", state=state, sex=sex, source_age=age_band,
                count=count, occupancy="vacant",
                entry_stream="overseas_slot", age_base_months=0,
                age_spread_months=0, entry_age_months=entry_age,
            ))

    plan = StatePlan(
        scale=scale,
        divisor=divisor,
        groups=tuple(groups),
        present_counts=present.counts,
        present_errors=present.errors,
        birth_counts=births.counts,
        overseas_counts=overseas.counts,
        present_slots=present.total,
        birth_slots=births.total,
        overseas_slots=overseas.total,
    )
    if sum(group.count for group in plan.groups) != plan.total_slots:
        raise AssertionError("compressed group counts do not equal the pool size")
    return plan


def _column_values(plan: StatePlan, column: str) -> Iterable[object]:
    ordinal = 0
    for group in plan.groups:
        for index in range(group.count):
            if column == "occupancy":
                yield group.occupancy
            elif column == "event":
                yield "none_"
            elif column == "sex":
                yield group.sex
            elif column == "age_months":
                yield group.age_months(index)
            elif column == "event_age_months":
                yield 0
            elif column == "generation":
                yield 0
            elif column == "entry_stream":
                yield group.entry_stream
            elif column == "entry_age_months":
                yield group.entry_age_months
            elif column == "area":
                yield group.state
            elif column == "prev_area":
                yield "none_"
            elif column == "slot_resource":
                yield ordinal
            else:
                raise ValueError(f"unknown PersonSlot column {column!r}")
            ordinal += 1


def _require_model_schema(model: dict) -> None:
    if model.get("name") != "australian_population":
        raise ValueError("paired model must be named 'australian_population'")
    tables = {
        (box["name"], table["name"]): table
        for box in model.get("boxes", []) for table in box.get("tables", [])
    }
    if set(tables) != {PERSON_TABLE, RESOURCE_TABLE}:
        raise ValueError(
            "Australian model must contain exactly demographic.person_slot and "
            "demographic.slot_resource"
        )
    person = tables[PERSON_TABLE]
    if tuple(attr["name"] for attr in person["attrs"]) != PERSON_COLUMNS:
        raise ValueError("Australian PersonSlot column contract changed")
    if tables[RESOURCE_TABLE]["attrs"]:
        raise ValueError("Australian SlotResource must have an empty schema")

    expected_enums = {
        "occupancy": ["vacant", "present"],
        "event": ["none_", "birth", "death", "overseas_arrival",
                  "overseas_departure", "interstate_move"],
        "sex": ["male", "female"],
        "entry_stream": ["birth_slot", "overseas_slot", "retired_slot"],
        "area": ["nsw", "vic", "qld", "sa", "wa", "tas", "nt", "act"],
        "prev_area": ["none_", "nsw", "vic", "qld", "sa", "wa", "tas",
                      "nt", "act"],
    }
    attrs = {attr["name"]: attr["ty"] for attr in person["attrs"]}
    for name, variants in expected_enums.items():
        if attrs[name] != {"kind": "enum", "variants": variants}:
            raise ValueError(f"Australian enum contract changed for {name!r}")
    for name in ("age_months", "event_age_months", "generation",
                 "entry_age_months"):
        if attrs[name] != {"kind": "int"}:
            raise ValueError(f"Australian integer contract changed for {name!r}")
    if attrs["slot_resource"] != {"kind": "ref", "table": "slot_resource"}:
        raise ValueError("Australian slot_resource reference contract changed")


def _clear_json_array(source: str, key: str, start: int = 0) -> tuple[str, int]:
    """Replace one canonical JSON array value with ``[]``; return next offset."""
    marker = f'"{key}":['
    marker_at = source.find(marker, start)
    if marker_at < 0:
        raise ValueError(f"canonical model is missing {key!r}")
    opening = marker_at + len(marker) - 1
    depth = 0
    in_string = False
    escaped = False
    for index in range(opening, len(source)):
        character = source[index]
        if in_string:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                in_string = False
            continue
        if character == '"':
            in_string = True
        elif character == "[":
            depth += 1
        elif character == "]":
            depth -= 1
            if depth == 0:
                replaced = source[:opening] + "[]" + source[index + 1:]
                return replaced, opening + 2
    raise ValueError(f"canonical model has an unterminated {key!r} array")


def _specialized_model(model_path, row_count: int) -> tuple[dict, bytes]:
    """Specialise rows and omit feature-gated grouped views from the companion."""
    source = pathlib.Path(model_path).read_text(encoding="utf-8")
    model = json.loads(source)
    _require_model_schema(model)
    for table_name in (PERSON_TABLE[1], RESOURCE_TABLE[1]):
        pattern = re.compile(
            rf'(\{{"name":"{re.escape(table_name)}","size_hint":)\d+'
        )
        source, replacements = pattern.subn(rf'\g<1>{row_count}', source)
        if replacements != 1:
            raise ValueError(
                f"canonical model must contain exactly one size hint for "
                f"{table_name!r}; found {replacements}"
            )
    offset = 0
    for _model_box in model["boxes"]:
        source, offset = _clear_json_array(source, "grouped_views", offset)
    if source.find('"grouped_views":[', offset) >= 0:
        raise ValueError("canonical model has more grouped-view arrays than boxes")

    specialized = json.loads(source)
    _require_model_schema(specialized)
    expected = state_artifact.resized_model(
        model,
        {PERSON_TABLE: row_count, RESOURCE_TABLE: row_count},
    )
    for model_box in expected["boxes"]:
        model_box["grouped_views"] = []
    if specialized != expected:
        raise AssertionError(
            "companion specialization changed more than row counts and grouped views"
        )
    return specialized, source.encode("utf-8")


def write_plan(plan: StatePlan, model_path, output_path) -> dict[str, str]:
    """Write an artifact and its scale-specialised paired model."""
    model, model_bytes = _specialized_model(model_path, plan.total_slots)
    columns = {
        (PERSON_TABLE[0], PERSON_TABLE[1], column):
            (lambda column=column: _column_values(plan, column))
        for column in PERSON_COLUMNS
    }
    output = pathlib.Path(output_path)
    digest = state_artifact.write_state(output, model, columns)
    companion = state_artifact.write_companion_bytes(output, model_bytes)
    return {
        "artifact": str(output),
        "companion": str(companion),
        "state_hash": digest.digest,
        "sha256": state_artifact.raw_sha256(output),
    }


def _default_output(scale: str) -> pathlib.Path:
    if scale == "hundredth":
        return pathlib.Path("fixtures/state/australian_population_2010_hundredth.state")
    return HERE / "generated" / f"australian_population_2010_{scale}.state"


def build_report() -> str:
    """Render the deterministic initial-state evidence report."""
    sources = load_source_totals()
    plans = {scale: build_plan(scale, sources) for scale in SCALES}
    lines = [
        "# Australian initial state: 30 June 2010",
        "",
        "This report is generated from the committed ABS extracts by",
        "`python3 data/abs/build_state.py --write-report`. Counts are people at",
        "`full` scale and slots at the named reduced scale.",
        "",
        "## Pool arithmetic",
        "",
        "The two entrant streams are single-use. Every initially present or exited",
        "row is `retired_slot`, so internal movement and retired rows cannot consume",
        "the pre-classified entry pools.",
        "",
        "| component | observed requirement | 10% headroom | full slots |",
        "|---|---:|---:|---:|",
        f"| present ERP, 2010 | {sum(sources.erp.values()):,} | 0 | "
        f"{sum(sources.erp.values()):,} |",
        f"| registered births, 2010–2024 | {sources.births_required:,} | "
        f"{sources.births_headroom:,} | "
        f"{sources.births_required + sources.births_headroom:,} |",
        f"| overseas arrivals, 2010–2024 | {sources.overseas_required:,} | "
        f"{sources.overseas_headroom:,} | "
        f"{sources.overseas_required + sources.overseas_headroom:,} |",
        f"| **total** | **{sum(sources.erp.values()) + sources.births_required + sources.overseas_required:,}** | "
        f"**{sources.births_headroom + sources.overseas_headroom:,}** | "
        f"**{plans['full'].total_slots:,}** |",
        "",
        "Headroom is rounded up independently before scale reduction. It is a",
        "capacity choice, not a claim that saturation is impossible: PRD 0008 fails",
        "a calibration whose birth or overseas vacancy margin reaches zero or",
        "approaches it without an explicitly justified reserve.",
        "",
        "## Scale outputs",
        "",
        "| scale | divisor | present | birth slots | overseas slots | total rows |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for scale in SCALES:
        plan = plans[scale]
        lines.append(
            f"| `{scale}` | {plan.divisor} | {plan.present_slots:,} | "
            f"{plan.birth_slots:,} | {plan.overseas_slots:,} | "
            f"{plan.total_slots:,} |"
        )
    lines += [
        "",
        "The sum of separately rounded stream capacities can differ by one from",
        "rounding the combined full pool. Stream capacities are authoritative",
        "because each stream has its own eligibility guard and saturation metric.",
        "",
        "## ERP fidelity and rounding",
        "",
        "State and single-year age margins are first apportioned to the national",
        "target. A deterministic minimum-cost bipartite allocation then chooses",
        "cell floor-to-ceiling increments, maximising remainders with sorted cell",
        "keys as the exact tie-break.",
        "",
        "| scale | national total exact | all 8 state margins exact | all 101 age margins exact | cell error range (agents) | cell MAE | zero cells | one-agent cells |",
        "|---|---|---|---|---:|---:|---:|---:|",
    ]
    for scale in SCALES:
        plan = plans[scale]
        errors = list(plan.present_errors.values())
        mae = sum(abs(value) / plan.divisor for value in errors) / len(errors)
        lines.append(
            f"| `{scale}` | yes | yes | yes | "
            f"{min(errors) / plan.divisor:+.2f} to "
            f"{max(errors) / plan.divisor:+.2f} | {mae:.6f} | "
            f"{sum(value == 0 for value in plan.present_counts.values())} | "
            f"{sum(value == 1 for value in plan.present_counts.values())} |"
        )
    hundredth = plans["hundredth"]
    lines += [
        "",
        "At `full`, every one of the 1,616 `(state, sex, age)` cells equals the",
        "published ERP count exactly. At `hundredth`, NT has "
        f"{sum(value == 0 for (state, _sex, _age), value in hundredth.present_counts.items() if state == 'nt')} zero and "
        f"{sum(value == 1 for (state, _sex, _age), value in hundredth.present_counts.items() if state == 'nt')} one-agent cells; ACT has "
        f"{sum(value == 0 for (state, _sex, _age), value in hundredth.present_counts.items() if state == 'act')} zero and "
        f"{sum(value == 1 for (state, _sex, _age), value in hundredth.present_counts.items() if state == 'act')} one-agent cells. These are the accepted small-cell limitation and are never smoothed.",
        "",
        "## Entrant composition",
        "",
        f"The eight-state birth requirement is {sources.births_required:,}. The",
        f"national sex series contains {sources.births_sex['male']:,} male and",
        f"{sources.births_sex['female']:,} female births; its {sources.national_birth_residual:,}-person",
        "excess is the retained Australia/eight-state residual. Birth slots preserve",
        "the apportioned state and sex margins exactly.",
        "",
        f"Detailed NOM arrivals sum to {sum(sources.nom_detail.values()):,}, which is",
        f"{sources.nom_detail_residual:+,} relative to the separately published gross",
        "margin used for pool size. Overseas slots preserve apportioned state × sex",
        "and age-band margins exactly. `entry_age_months` is the published band's",
        "lower bound times 12, including 65+ as 780; no within-band observations are",
        "manufactured.",
        "",
        "Entrant composition is fixed when the artifact is built. Calibration fits",
        "birth and overseas *rates*, not their state, sex or age mix.",
        "",
        "## Row encoding",
        "",
        "Present rows are sorted by `(state, sex, age, within-cell index)`; their",
        "single-year ages are spread by `age * 12 + floor(index * 12 / cell_count)`.",
        "Birth groups follow in `(state, sex)` order, then overseas groups in",
        "`(state, sex, published age band)` order. Row ordinal is the Philox entity",
        "coordinate and the row's `slot_resource` reference. Every initial event is",
        "`none_`, every `prev_area` is `none_`, and every generation is 0.",
        "",
        "## Artifact evidence",
        "",
        "`state hash` is SHA-256 over `sembla.state-artifact/v1 || 0x00 || bytes`,",
        "the exact record printed by Rust `sembla state-hash`. `file SHA-256` is the",
        "ordinary digest used for file pinning.",
        "",
        "| scale | path | bytes | file SHA-256 | state hash |",
        "|---|---|---:|---|---|",
    ]
    for scale in SCALES:
        evidence = ARTIFACT_EVIDENCE[scale]
        lines.append(
            f"| `{scale}` | `{evidence['path']}` | {evidence['bytes']:,} | "
            f"`{evidence['sha256']}` | `{evidence['state_hash']}` |"
        )
    lines += [
        "",
        "`hundredth` and its paired model are committed. `full` and `tenth` are",
        "generated on demand under ignored `data/abs/generated/`. Each validation-safe",
        "companion changes the two table `size_hint` values and omits only the",
        "feature-gated `grouped_views`; parameters, dynamics and scalar views remain",
        "identical to the canonical execution model.",
        "The full regeneration, Rust hashes and negative row-count checks are recorded",
        "in `docs/evidence/australian-population/initial-state-2010/README.md`.",
        "",
        "## Cross-language conformance",
        "",
        "- The generic Python writer reproduces Rust's committed `refs_small.state`",
        "  bytes and frozen state hash exactly.",
        "- `AustralianPopulation.lean` generates 56 move and 336 mortality transitions",
        "  and its post-splice model passes Lean `checkModel`.",
        "- Public `sembla validate` accepts both the validation-safe `.state.model.json`",
        "  companion and the canonical feature-bearing executable plan. A structural",
        "  test proves that only grouped views and scale row counts differ.",
        "- `cargo test -p sembla-cli --test australian_population` checks schema,",
        "  rows, stream counts, ordinals, companion equivalence and the Rust state",
        "  hash, then proves identity and retirement for every slot across all 24 golden",
        "  boundaries, exact interstate conservation, state/national stock-flow identities,",
        "  closed accounting and deterministic replay.",
        "- A zero-tick export is not honest because summaries cannot reduce an empty",
        "  run. Conformance therefore uses Rust `state-hash` plus exact loader values,",
        "  rather than pretending a one-tick changed state is a round trip.",
        "",
    ]
    return "\n".join(lines)


def write_report(path=EXTRACTS / "initial-state-2010.md") -> None:
    pathlib.Path(path).write_text(build_report(), encoding="utf-8", newline="")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scale", choices=sorted(SCALES))
    parser.add_argument("--model", type=pathlib.Path,
                        help="standalone PRD 0004A exported model JSON")
    parser.add_argument("--out", type=pathlib.Path)
    parser.add_argument(
        "--plan-only", action="store_true",
        help="validate allocations and print counts without writing an artifact",
    )
    parser.add_argument(
        "--write-report", action="store_true",
        help="regenerate extracts/initial-state-2010.md without building artifacts",
    )
    args = parser.parse_args(argv)

    if args.write_report:
        write_report()
        print("wrote extracts/initial-state-2010.md")
        return 0
    if args.scale is None:
        parser.error("--scale is required unless --write-report is used")

    plan = build_plan(args.scale)
    print(
        f"scale={plan.scale} present={plan.present_slots} "
        f"birth={plan.birth_slots} overseas={plan.overseas_slots} "
        f"total={plan.total_slots}"
    )
    if args.plan_only:
        return 0
    if args.model is None:
        parser.error("--model is required unless --plan-only is used")
    output = args.out or _default_output(args.scale)
    metadata = write_plan(plan, args.model, output)
    print(
        f"state sha256 {state_artifact.STATE_HASH_DOMAIN} "
        f"{metadata['state_hash']}"
    )
    print(f"file sha256 {metadata['sha256']}")
    print(f"companion_model={metadata['companion']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
