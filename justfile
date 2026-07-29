# List available recipes
default:
    @just --list

# One-time install of all dev tools (idempotent, safe to re-run)
setup:
    cargo install cargo-binstall
    cargo binstall -y bacon           # background checker
    cargo binstall -y dioxus-cli      # `dx serve --hot-patch` (hotpatching)
    cargo binstall -y cargo-nextest   # faster test runner
    cargo binstall -y cargo-llvm-cov  # code coverage
    # bevy_cli is alpha — not on crates.io, released via GitHub only
    cargo binstall -y --git https://github.com/TheBevyFlock/bevy_cli --version 0.1.0-alpha.2 --locked bevy_cli
    cargo binstall -y cargo-deny      # supply-chain checks
    cargo binstall -y cargo-machete   # dead-dependency detection
    bevy lint install main            # bevy_lint component (own pinned nightly, slow first time)
    git config core.hooksPath .githooks  # enable committed git hooks

# Run all tests with the coverage ratchet. Raise the floor as coverage rises (it only goes up).
COVERAGE_FLOOR := "90"
test:
    cargo llvm-cov nextest --no-tests=pass --fail-under-lines {{COVERAGE_FLOOR}}
    cargo test --doc

lint:
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    bevy lint

# LLM smells review on staged diff: see heuristics.toml
smells:
    #!/bin/sh
    DIFF=$(git diff --cached --unified=5 -- '*.rs' '*.toml' '*.ron')
    if [ -z "$DIFF" ]; then echo "smells: no staged Rust/config changes"; exit 0; fi
    PROMPT="You are the smells reviewer for this repo. Read heuristics.toml (the registry of judgment-call rules). Review ONLY the staged diff provided on stdin against those rules. Do not invent findings; absence of violations is a valid result. For each violation output: rule id, file and line, and the smallest fix. Then output the verdict as the FINAL line, exactly PASS or FAIL."
    OUT=$(printf '%s\n' "$DIFF" | claude -p "$PROMPT")
    printf '%s\n' "$OUT"
    printf '%s\n' "$OUT" | tail -1 | grep -qx 'PASS'

# LLM code review: on staged diff, functional defects only
review:
    #!/bin/sh
    DIFF=$(git diff --cached --unified=10 -- '*.rs' '*.toml' '*.ron')
    if [ -z "$DIFF" ]; then echo "review: no staged Rust/config changes"; exit 0; fi
    PROMPT="You are a code reviewer. Review ONLY the staged diff provided on stdin for real defects: logic errors, panics/crashes on reachable paths, wrong math, ordering/timing bugs, API misuse, missing edge cases, broken invariants. Do NOT report style, formatting, or convention issues (clippy and the smells gate own those). Read surrounding files if needed to verify a suspicion — do not report unverified guesses. For each confirmed defect output: file and line, the failure scenario (input/state -> wrong outcome), and the smallest fix. Then output the verdict as the FINAL line, exactly PASS or FAIL."
    OUT=$(printf '%s\n' "$DIFF" | claude -p "$PROMPT")
    printf '%s\n' "$OUT"
    printf '%s\n' "$OUT" | tail -1 | grep -qx 'PASS'

# Supply chain: advisories, licenses, duplicate versions, dead deps
audit:
    cargo deny check
    cargo machete

# Exhaustive gate (called by .githooks/pre-commit; mirrors CI)
precommit: lint test audit smells review

run:
    dx serve --hot-patch --features dev

# ─────────────────────────────────────────────────────────────────────
# DELETABLE — demo recipes for this repo's examples. When using this
# repo as a template, delete everything below (and the demo assets/code
# each recipe mentions).
# ─────────────────────────────────────────────────────────────────────

# DEMO asset hot reload: swap box.png ↔ box-alt.png (red↔blue) while the game runs
swap-asset:
    #!/bin/sh
    cd assets/images
    mv box.png box-tmp.png && mv box-alt.png box.png && mv box-tmp.png box-alt.png

# DEMO probe: measure hotpatch latency end-to-end (starts the game, edits code, times the patch)
probe-hotpatch:
    #!/bin/sh
    LOG=/tmp/bevy-dx-probe.log
    rm -f "$LOG"
    dx serve --hot-patch --features dev >"$LOG" 2>&1 &
    DX_PID=$!
    trap 'kill $DX_PID 2>/dev/null; sed -i "" "/hotpatch-probe/d" src/lib.rs' EXIT
    echo "waiting for initial build + app launch (log: $LOG)..."
    i=0
    until grep -qiE "app launched|serving|hot.?reload" "$LOG"; do
        sleep 1; i=$((i+1))
        [ $i -gt 300 ] && { echo "TIMEOUT waiting for launch"; tail -5 "$LOG"; exit 1; }
    done
    sleep 2
    echo "editing src/lib.rs..."
    START=$(date +%s)
    printf '\n// hotpatch-probe marker\n' >> src/lib.rs
    i=0
    until grep -qiE "hot.?patch(ed)?" "$LOG"; do
        sleep 1; i=$((i+1))
        [ $i -gt 60 ] && { echo "TIMEOUT: no patch within 60s"; tail -5 "$LOG"; exit 1; }
    done
    echo "PROBE RESULT: patch observed ~$(( $(date +%s) - START ))s after edit"
    grep -iE "hot.?patch" "$LOG" | tail -3
