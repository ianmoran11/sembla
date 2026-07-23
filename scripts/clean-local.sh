#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: bash scripts/clean-local.sh [--apply]

Preview or remove only these rebuildable local caches:
  target/
  frontend/.lake/
  .pytest_cache/
  calibration/npe/.venv/
  calibration/npe/**/__pycache__/

The default is a dry run. Pass --apply to delete validated allowlisted paths.
Applying cleanup means Rust and Lean outputs must be rebuilt and Python
packages may need to be reinstalled from the repository's pinned environment.
EOF
}

fail() {
    echo "error: $*" >&2
    exit 1
}

case "$#" in
    0)
        apply=false
        ;;
    1)
        case "$1" in
            --apply)
                apply=true
                ;;
            --help)
                usage
                exit 0
                ;;
            *)
                echo "error: unknown option: $1" >&2
                usage >&2
                exit 2
                ;;
        esac
        ;;
    *)
        echo "error: expected at most one option; unknown option: $2" >&2
        usage >&2
        exit 2
        ;;
esac

command -v git >/dev/null 2>&1 || fail "git is required to protect tracked files"

[[ -z "${GIT_INDEX_FILE:-}" ]] || \
    fail "refusing Git repository/index override: GIT_INDEX_FILE"
[[ -z "${GIT_DIR:-}" ]] || \
    fail "refusing Git repository/index override: GIT_DIR"
[[ -z "${GIT_WORK_TREE:-}" ]] || \
    fail "refusing Git repository/index override: GIT_WORK_TREE"
[[ -z "${GIT_COMMON_DIR:-}" ]] || \
    fail "refusing Git repository/index override: GIT_COMMON_DIR"
[[ -z "${GIT_IMPLICIT_WORK_TREE:-}" ]] || \
    fail "refusing Git repository/index override: GIT_IMPLICIT_WORK_TREE"

script_source="${BASH_SOURCE[0]}"
script_name="$(basename -- "$script_source")"
script_input_dir="$(dirname -- "$script_source")"
logical_script_dir="$(cd -L -- "$script_input_dir" && pwd -L)" || \
    fail "cannot resolve the script directory"
physical_script_dir="$(cd -P -- "$script_input_dir" && pwd -P)" || \
    fail "cannot canonicalize the script directory"
logical_script="$logical_script_dir/$script_name"
canonical_script="$physical_script_dir/$script_name"

if [[ -L "$logical_script" || "$logical_script" != "$canonical_script" ]]; then
    fail "refusing an unexpected or symlinked repository root/script path: $logical_script"
fi
if [[ ! -f "$canonical_script" ]]; then
    fail "cleanup script is not a regular file: $canonical_script"
fi

repo_root="$(cd -P -- "$physical_script_dir/.." && pwd -P)"
expected_script="$repo_root/scripts/clean-local.sh"
if [[ "$canonical_script" != "$expected_script" ]]; then
    fail "cleanup script must be located at scripts/clean-local.sh under its repository root"
fi

git_root_input="$(git -C "$repo_root" rev-parse --show-toplevel 2>/dev/null)" || \
    fail "scripts/clean-local.sh is not inside a Git working tree"
git_root="$(cd -P -- "$git_root_input" && pwd -P)" || \
    fail "cannot canonicalize the Git working-tree root"
if [[ "$git_root" != "$repo_root" ]]; then
    fail "unexpected repository root: script resolved to $repo_root but Git resolved to $git_root"
fi

is_allowlisted() {
    local relative="$1"
    case "$relative" in
        target|frontend/.lake|.pytest_cache|calibration/npe/.venv)
            return 0
            ;;
        calibration/npe/__pycache__|calibration/npe/*/__pycache__)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

is_protected() {
    local relative="$1"
    local surrounded="/$relative/"
    local base="${relative##*/}"

    case "$surrounded" in
        */.git/*|*/.piprd/*|*/.pi-subagents/*|*/fixtures/*|*/examples/*)
            return 0
            ;;
        /calibration/npe/artifacts/*|/spikes/*/artifacts/*|/spikes/*/evidence/*)
            return 0
            ;;
        /spikes/precision/infra/*|/spikes/precision/infra-hyperstack/*|/spikes/precision/infra-vultr/*)
            return 0
            ;;
        */.terraform/*|*/terraform/*)
            return 0
            ;;
    esac

    case "$base" in
        *.tf|*.tf.json|*.tfvars|*.tfvars.json|*.tfstate|*.tfstate.*|*.tfplan|.terraform.lock.hcl)
            return 0
            ;;
    esac

    return 1
}

validate_relative() {
    local relative="$1"
    case "$relative" in
        ""|/*|.|..|*/../*|../*|*/..|*/./*|./*|*/.)
            fail "invalid cleanup candidate: $relative"
            ;;
    esac
    if ! is_allowlisted "$relative"; then
        fail "path is outside the closed cleanup allowlist: $relative"
    fi
    if is_protected "$relative"; then
        fail "allowlisted path unexpectedly overlaps a protected path: $relative"
    fi
    return 0
}

tracked_files_for() {
    local relative="$1"
    git -C "$repo_root" ls-files -- "$relative"
}

validate_existing_candidate() {
    local relative="$1"
    local candidate="$repo_root/$relative"
    local canonical
    local canonical_relative
    local tracked

    validate_relative "$relative"

    if [[ -L "$candidate" ]]; then
        canonical="$(cd -P -- "$candidate" 2>/dev/null && pwd -P)" || \
            fail "cannot canonicalize symlinked cleanup candidate: $relative"
        case "$canonical" in
            "$repo_root"/*)
                canonical_relative="${canonical#"$repo_root"/}"
                if is_protected "$canonical_relative"; then
                    fail "candidate resolves to protected path: $relative -> $canonical_relative"
                fi
                ;;
            *)
                fail "cleanup candidate escapes the repository root: $relative -> $canonical"
                ;;
        esac
        fail "cleanup candidates may not be symlinks: $relative"
    fi

    canonical="$(cd -P -- "$candidate" 2>/dev/null && pwd -P)" || \
        fail "cannot canonicalize cleanup candidate: $relative"
    case "$canonical" in
        "$repo_root"/*)
            canonical_relative="${canonical#"$repo_root"/}"
            ;;
        *)
            fail "cleanup candidate escapes the repository root: $relative -> $canonical"
            ;;
    esac

    if is_protected "$canonical_relative"; then
        fail "candidate resolves to protected path: $relative -> $canonical_relative"
    fi
    if ! is_allowlisted "$canonical_relative"; then
        fail "canonical candidate is outside the closed cleanup allowlist: $relative -> $canonical_relative"
    fi
    if [[ "$canonical_relative" != "$relative" ]]; then
        fail "candidate changed identity while canonicalizing: $relative -> $canonical_relative"
    fi

    tracked="$(tracked_files_for "$relative")"
    if [[ -n "$tracked" ]]; then
        fail "refusing to remove $relative because it contains tracked files"
    fi
    return 0
}

candidate_relatives=(
    "target"
    "frontend/.lake"
    ".pytest_cache"
    "calibration/npe/.venv"
)

npe_root="$repo_root/calibration/npe"
if [[ -L "$npe_root" ]]; then
    fail "refusing symlinked Python calibration root: calibration/npe"
fi
if [[ -d "$npe_root" ]]; then
    while IFS= read -r -d '' cache_path; do
        candidate_relatives+=("${cache_path#"$repo_root"/}")
    done < <(
        find -P "$npe_root" \
            \( -type d \( \
                -name .venv -o \
                -name artifacts -o \
                -name fixtures -o \
                -name examples -o \
                -name .git -o \
                -name .piprd -o \
                -name .pi-subagents -o \
                -name .terraform -o \
                -name terraform \
            \) -prune \) -o \
            \( -type d -name __pycache__ -print0 -prune \)
    )
fi

# Validate every existing path before producing a deletion plan. This prevents a
# later unsafe candidate from causing a partially applied cleanup.
for relative in "${candidate_relatives[@]}"; do
    validate_relative "$relative"
    candidate="$repo_root/$relative"
    if [[ -e "$candidate" || -L "$candidate" ]]; then
        validate_existing_candidate "$relative"
    fi
done

if [[ "$apply" == false ]]; then
    echo "Dry run only; nothing will be deleted. Re-run with --apply to remove validated caches."
else
    echo "Apply mode: removing validated rebuildable local caches."
fi
echo "Rust and Lean outputs will need rebuilding; Python dependencies may need reinstalling."

for relative in "${candidate_relatives[@]}"; do
    candidate="$repo_root/$relative"
    display="$relative/"
    if [[ ! -e "$candidate" && ! -L "$candidate" ]]; then
        echo "$display: missing"
        continue
    fi

    size="$(du -sh -- "$candidate" 2>/dev/null | awk '{print $1}' || true)"
    if [[ -z "$size" ]]; then
        size="unknown"
    fi

    if [[ "$apply" == false ]]; then
        echo "$display: exists (approximately $size)"
        continue
    fi

    # Re-check immediately before removal in case the path changed after the
    # complete plan was validated. rm receives exact paths and does not follow
    # symlinks encountered below an allowlisted directory.
    validate_existing_candidate "$relative"
    echo "$display: removing (approximately $size)"
    rm -rf -- "$candidate"
done

if [[ "$apply" == false ]]; then
    echo "Dry run complete; no files were removed."
else
    echo "Local cache cleanup complete."
fi
