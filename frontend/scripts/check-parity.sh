#!/usr/bin/env bash
set -euo pipefail
frontend_root="$(cd "$(dirname "$0")/.." && pwd)"
repo_root="$(cd "$frontend_root/.." && pwd)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/sembla-lean.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

canonical_models=(
  "reversible_ctmc|reversible_ctmc.json|20|2|tick,count:chain.particle.phase=A,count:chain.particle.phase=B,fired:chain.move_ab,fired:chain.move_ba,deferred_total"
  "radioactive_decay_chain|radioactive_decay_chain.json|30|3|tick,count:decay.atom.nuclide=Parent,count:decay.atom.nuclide=Daughter,count:decay.atom.nuclide=Stable,fired:decay.parent_decay,fired:decay.daughter_decay,deferred_total"
  "sis_importation|sis_importation.json|30|2|tick,count:epidemic.person.health=S,count:epidemic.person.health=I,fired:epidemic.infect,fired:epidemic.recover,deferred_total"
  "seirs_waning|seirs_waning.json|40|4|tick,count:epidemic.person.health=S,count:epidemic.person.health=E,count:epidemic.person.health=I,count:epidemic.person.health=R,fired:epidemic.expose,fired:epidemic.progress,fired:epidemic.recover,fired:epidemic.wane,deferred_total"
  "noisy_voter|noisy_voter.json|30|2|tick,count:opinions.agent.opinion=A,count:opinions.agent.opinion=B,fired:opinions.adopt_b,fired:opinions.adopt_a,deferred_total"
)

canonical_aliases=(
  "reversibleCtmc|reversible_ctmc.json"
  "Sembla.Models.reversibleCtmc|reversible_ctmc.json"
  "Sembla.Models.reversible_ctmc|reversible_ctmc.json"
  "Sembla/Models/reversibleCtmc|reversible_ctmc.json"
  "Sembla/Models/reversible_ctmc|reversible_ctmc.json"
  "radioactiveDecayChain|radioactive_decay_chain.json"
  "Sembla.Models.radioactiveDecayChain|radioactive_decay_chain.json"
  "Sembla.Models.radioactive_decay_chain|radioactive_decay_chain.json"
  "Sembla/Models/radioactiveDecayChain|radioactive_decay_chain.json"
  "Sembla/Models/radioactive_decay_chain|radioactive_decay_chain.json"
  "sisImportation|sis_importation.json"
  "Sembla.Models.sisImportation|sis_importation.json"
  "Sembla.Models.sis_importation|sis_importation.json"
  "Sembla/Models/sisImportation|sis_importation.json"
  "Sembla/Models/sis_importation|sis_importation.json"
  "seirsWaning|seirs_waning.json"
  "Sembla.Models.seirsWaning|seirs_waning.json"
  "Sembla.Models.seirs_waning|seirs_waning.json"
  "Sembla/Models/seirsWaning|seirs_waning.json"
  "Sembla/Models/seirs_waning|seirs_waning.json"
  "noisyVoter|noisy_voter.json"
  "Sembla.Models.noisyVoter|noisy_voter.json"
  "Sembla.Models.noisy_voter|noisy_voter.json"
  "Sembla/Models/noisyVoter|noisy_voter.json"
  "Sembla/Models/noisy_voter|noisy_voter.json"
)

cd "$frontend_root"
lake build
bash scripts/test-negative.sh
lake exe sembla-export sir "$tmp/sir.json"
lake exe sembla-export Sembla.Models.sirPolicy "$tmp/sir_policy.json"
lake exe sembla-export observations "$tmp/observations.json"
for specification in "${canonical_models[@]}"; do
  IFS='|' read -r export_name file _ _ _ <<<"$specification"
  lake exe sembla-export "$export_name" "$tmp/$file"
done
alias_index=0
for specification in "${canonical_aliases[@]}"; do
  IFS='|' read -r export_name file <<<"$specification"
  lake exe sembla-export "$export_name" "$tmp/alias-$alias_index-$file"
  alias_index=$((alias_index + 1))
done

cd "$repo_root"
cargo build --quiet -p sembla-cli
sembla="$repo_root/target/debug/sembla"
for file in sir.json sir_policy.json observations.json; do
  "$sembla" validate "examples/$file"
  "$sembla" validate "$tmp/$file"
  cmp "examples/$file" "$tmp/$file"
  "$sembla" diff-ir "examples/$file" "$tmp/$file"
done
alias_index=0
for specification in "${canonical_aliases[@]}"; do
  IFS='|' read -r _ file <<<"$specification"
  "$sembla" diff-ir "examples/$file" "$tmp/alias-$alias_index-$file"
  alias_index=$((alias_index + 1))
done
for specification in "${canonical_models[@]}"; do
  IFS='|' read -r _ file ticks state_columns expected_header <<<"$specification"
  checked="examples/$file"
  exported="$tmp/$file"
  "$sembla" validate "$checked"
  "$sembla" validate "$exported"
  cmp "$checked" "$exported"
  "$sembla" diff-ir "$checked" "$exported"

  for source in checked exported; do
    model="$checked"
    if [[ "$source" == exported ]]; then model="$exported"; fi
    for repeat in first second; do
      "$sembla" run "$model" --population 1000 --seed 55 --ticks "$ticks" \
        --out "$tmp/${file%.json}-$source-$repeat.csv" \
        >"$tmp/${file%.json}-$source-$repeat.hashes"
    done
  done
  for candidate in \
    "$tmp/${file%.json}-checked-second.csv" \
    "$tmp/${file%.json}-exported-first.csv" \
    "$tmp/${file%.json}-exported-second.csv"; do
    cmp "$tmp/${file%.json}-checked-first.csv" "$candidate"
  done
  for candidate in \
    "$tmp/${file%.json}-checked-second.hashes" \
    "$tmp/${file%.json}-exported-first.hashes" \
    "$tmp/${file%.json}-exported-second.hashes"; do
    cmp "$tmp/${file%.json}-checked-first.hashes" "$candidate"
  done
  grep -Fqx "$expected_header" "$tmp/${file%.json}-checked-first.csv"
  awk -F, -v states="$state_columns" '
    /^#/ || $1 == "tick" { next }
    {
      total = 0
      for (column = 2; column < 2 + states; column++) total += $column
      if (total != 1000) exit 1
      if ($2 < 1000) changed = 1
    }
    END { if (!changed) exit 1 }
  ' "$tmp/${file%.json}-checked-first.csv"
done

# End-to-end parity traverses population serialization, observations, and the
# executor rather than stopping at JSON normalization.
"$sembla" synth-pop --persons 1000 --employers 50 --initial-infected 10 --seed 12 --out "$tmp/pop.bin" >/dev/null
for stem in sir sir_policy; do
  "$sembla" run "examples/$stem.json" --population "$tmp/pop.bin" --seed 55 --ticks 20 \
    --out "$tmp/$stem-fixture.csv" >"$tmp/$stem-fixture.hashes"
  "$sembla" run "$tmp/$stem.json" --population "$tmp/pop.bin" --seed 55 --ticks 20 \
    --out "$tmp/$stem-exported.csv" >"$tmp/$stem-exported.hashes"
  cmp "$tmp/$stem-fixture.csv" "$tmp/$stem-exported.csv"
  cmp "$tmp/$stem-fixture.csv.summaries.csv" "$tmp/$stem-exported.csv.summaries.csv"
  cmp "$tmp/$stem-fixture.hashes" "$tmp/$stem-exported.hashes"
done
echo "Lean export, validation, canonical-byte/normalized parity, observation parity, and execution-hash parity passed"
