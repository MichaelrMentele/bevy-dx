# README

The goal of this repo is to provide an authoritative example of an optimal Bevy devx workflow that provides:
1. the tightest feedback loops possible
2. loops from dev to CI to production
3. a deterministic march towards correctness

## The Loops

```
╭─ DEV ──────────────────────────────────────────── ms–5s ─╮
│  editor checks · format on save                     ~ms  │
│  hot asset serving                                  ~ms  │
│  hotpatch (dx serve)                                <2s  │
│  restart (fast rebuild)                            2–5s  │
│  bacon: clippy + tests in background              async  │
│                                                          │
│  ╭─ COMMIT ───────────────────────────────── seconds ─╮  │
│  │  hook = just precommit       fmt · clippy · tests  │  │
│  │                                                    │  │
│  │  ╭─ MERGE  (CI) ────────────────────── minutes ─╮  │  │
│  │  │  same gate on a clean machine                │  │  │
│  │  │  release build · deny · release              │  │  │
│  │  │                                              │  │  │
│  │  │  ╭─ PRODUCTION ────────────── continuous ─╮  │  │  │
│  │  │  │  crash reports · diagnostics           │  │  │  │
│  │  │  │  seed + inputs = replayable            │  │  │  │
│  │  │  ╰─────────────────┬──────────────────────╯  │  │  │
│  │  ╰────────────────────┼─────────────────────────╯  │  │
│  ╰───────────────────────┼────────────────────────────╯  │
│   ⟲ crashes · thresholds · player feedback → back to DEV │
╰──────────────────────────────────────────────────────────╯
```

Status: DEV and COMMIT are built and live in this repo. MERGE (CI),
RELEASE, and PRODUCTION are planned — the diagram shows the target,
not the current state.

## How to use this template

1. Clone (TODO once on GitHub: `bevy new my_game --template gh:<repo>`), rename the crate in `Cargo.toml`
2. `cargo install just && just setup` — tools, toolchain, git hooks
3. Try and then delete the demos: everything below the `DELETABLE` fence in the `justfile`,
   the demo assets (`assets/images/box*.png`, `assets/config/demo_box.ron`), and the
   demo systems/config in `src/lib.rs`
4. Keep: the workflow recipes, `.zed/`, `.githooks/`, `rust-toolchain.toml`, profiles in `Cargo.toml`

## Layout

Every crate is a lib exposing a Plugin, plus a 3-line main:
```
src/
  main.rs             # 3 lines: App + GamePlugin. No logic — untestable by definition.
  lib.rs              # exposes GamePlugin
  dev.rs              # dev-feature-only tooling: inspector, FPS overlay, BRP
  plugins/            # TODO (convention, not created yet): one module per concern, each a Plugin
tests/                # black-box only: internal-remotes/ (BRP harness — placeholder),
                      #   user/ (SOPs for human/agent play)
tools/                # first-party remote clients (editor, replay viewer, ...) — stub, see tools/README.md
assets/
  config/             # tunables: *.ron (hot-reloaded), split by domain (demo_box.ron)
  images/
  audio/
justfile              # every workflow command — run `just` to list them
heuristics.toml       # smells registry: judgment-call rules for LLM/human review
Cargo.toml            # deps + [features] dev gate + [lints.clippy] strictness + fast profiles
Cargo.lock            # committed: exact dep versions for reproducible builds
clippy.toml           # lint config: disallowed-methods (rand ban), unwrap-ok-in-tests
deny.toml             # supply-chain policy: license allowlist, advisories
rust-toolchain.toml   # pins compiler + components (clippy, rustfmt, rust-src, llvm-tools)
bacon.toml            # background watch loop: clippy-all default, nextest job
LICENSE               # MIT
.zed/settings.json    # project editor settings: format-on-save, clippy inline, own target dir
.zed/debug.json       # step-debug launch config (builds with --features dev)
.githooks/pre-commit  # runs `just precommit` (enabled by `just setup` via core.hooksPath)
```

## DEV

*The inner loop (ms–5s)*

### Setup

- `rust-toolchain.toml` + committed `Cargo.lock` — pins the rust toolchain
- one-time tool installs (recipes live in the `justfile`):
  ```sh
  cargo install just   # bootstrap the task runner
  just setup           # installs bacon, dioxus-cli, nextest, llvm-cov, bevy_cli, bevy_lint, cargo-deny, cargo-machete
  ```
  `just setup` also enables the committed git hooks (`git config core.hooksPath .githooks`)
- Daily loops: run `just` to list them — the justfile is the source of truth

### Editor
- inline errors as you type (rust-analyzer)
- project settings committed in `.zed/settings.json` (they override user-level settings):
  - `format_on_save: "on"` for Rust — enforced per-project so a global "off" can't break the workflow
  - `check.command: "clippy"` — inline lints, not just compile errors
  - `cargo.targetDir: true` — own target dir, no build-lock stalls vs cargo/bacon
  - `cargo.features: ["dev"]` — analyze dev-gated code too
- step debugging (LLDB) — launch config committed in `.zed/debug.json` (builds with `dev`)
  - per-frame breakpoints retrigger every frame — add a condition (e.g. `time.elapsed_secs() > 3.0`)
  - calmer variant: debug a `#[test]` via its gutter icon — headless, deterministic
  - **separate launch from `just run` (dx owns that process) — no hotpatching while debugging**

### Hot Reloading
- hot patching (dx serve --hot-patch)
  - only patches function bodies that existed at launch i.e. new systems/components/startup logic force a restart
  - won't affect things that already ran like `setup` — patched code takes effect at its *next call*, and startup code is never called again
- keep the restart path fast
  - structural change while hotpatching: press `r` in the dx TUI — full rebuild + relaunch
  - [fast compile config](https://github.com/TheBevyFlock/bevy_new_2d/blob/main/Cargo.toml)
  - dev profile / dynamic linking (broken on Bevy 0.19 — no published dylib)
- asset hot reload — via the `bevy/file_watcher` cargo feature (dev-only)
  - AssetServer watches `assets/` with OS file notifications
  - auto-reloads changed files into all live handles, no code needed
  - try it: `just run`, then `just swap-asset` in another terminal — the box texture
    (`assets/images/box.png`) flips red↔blue in the running game, no rebuild

### Live Debugging
- world inspector (bevy-inspector-egui) — browse/edit every entity, component,
  and resource in the running game; toggle with ` (backquote) during `just run`
- FPS overlay (bevy_dev_tools)
- gizmos — Bevy's built-in debug drawing, available in any system via `Gizmos`
- physics debug rendering — Avian's `PhysicsDebugPlugin` is wired into `src/dev.rs` behind the `dev` feature (collider/contact wireframes, never in release)

### Background Checks
- `bacon` watches files, re-runs checks on every save (config: `bacon.toml`)
  - default job is `clippy-all` (same lint set as the gate, advisory — never blocks)
  - `t` switches to tests (nextest), `c` back to clippy — live, per keypress

### Lints
- clippy — max strict: `pedantic` + `nursery` groups in `Cargo.toml [lints.clippy]`,
  restriction cherry-picks (`unwrap_used`, `print_stdout`, ...), Bevy-idiom allows
  (`needless_pass_by_value`, `type_complexity`); lint *config* in `clippy.toml`
  (`disallowed-methods` rand ban, unwrap-ok-in-tests)
- bevy_lint — Bevy-semantic lints (panicking query methods, unit bundles, ...), in `just lint`.
  Installed from main by `just setup` (released version lags Bevy; own pinned nightly)

### Testing

Two boundaries — each has a live example in this repo:
1. In-process = unit scope, in `src/`. **The plugin is the unit**: exercise it
   through a headless App (`headless_app()` in `lib.rs` tests) — wiring bugs
   surface as behavior failures; never assert configuration (tautologies).
   Pure functions get doctests (`box_x`). Shipped data gets a parseability
   test (`shipped_config_ron_parses` — a broken RON fails silently at runtime).
2. `tests/` = black box only: the build is an artifact, not a library.
   - `tests/internal-remotes/` (placeholder) — spawn the binary, drive BRP, assert, kill.
     TODO: the `game/*` facade tests speak (contract, never raw queries).
     Agents use the same door; multiplayer = N instances, one port each
   - `tests/user/` — SOPs for human/agent play; computer-use agents later (release ring)

Methodology:
- Coverage ratchet — `just test` runs the suite instrumented and fails below
  `COVERAGE_FLOOR` (justfile). Raise the floor as coverage rises — it only
  moves up.
- Test through public API with the query/command split — the rule is `test-the-real-claim` in `heuristics.toml`.
- A TDD style workflow with agentic coding under those constraints is recommended.
- TODO: tutorial on e2e TDD — pick the cheapest boundary that proves the claim

### Invariant Testing

Determinism makes these possible — the code rules are `sim-in-fixed-update` and
`seeded-rng-only` in `heuristics.toml`; these are the mechanisms that enforce
and exploit the outcome:
- TODO Schedule ambiguity detection — `Warn` in dev runs, `Error` in test apps
- TODO determinism test: same seed + N ticks → identical world snapshot
- TODO Property-based testing (proptest) — e.g. "any seed produces a valid, connected map"
- Fuzz testing (planned)
  - cargo-fuzz on parse boundaries (save files, config RON, network messages)
  - monkey testing — seeded random input streams for thousands of ticks; assert no panic, no NaN positions, no stuck states; failing stream = replayable repro

### Performance (planned — not implemented)
Trends, not pass/fail — no assertion can own "fast enough" on shared hardware:
- criterion benches on hot systems — relative regression vs saved baseline (CI-safe)
- absolute budgets (FPS p95, RAM) validated on target hardware per release —
  these are the targets PRODUCTION's threshold alerts compare against

## COMMIT
*The local gate (seconds)*

- `.githooks/pre-commit` runs exhaustive `just precommit`
- `just smells` — conventions oracle: checks the diff against `heuristics.toml`
- `just review` — pre-view of PR code review

## MERGE (planned — not implemented)
*Merge to main checks (minutes)*

The intent: CI runs the same `just precommit` gate on a clean machine, plus
release build and `cargo deny`. No workflow exists yet.

## RELEASE (planned — not implemented)

**From main to users machines.**

## PRODUCTION (planned — not implemented)
*The product return loop — nothing below is built yet (ship-time work)*

Three channels feed signal back into DEV:
- **crashes** — panic hook ships seed + input log (Sentry aggregates);
  determinism replays it as a failing test, so that bug class can't ship again
- **diagnostics** (opt-in) — threshold crossed (FPS p95, memory, load time)
  opens a regression with the offending build pinned; the fix lands as a bench
  or budget test, so it's now gated
- **player feedback** — reports/reviews/telemetry triaged into playtest
  scenarios; the one channel a human must close

## References

Templates & mega-examples:
- [bevy_new_2d](https://github.com/TheBevyFlock/bevy_new_2d) — blessed 2D template: screens/menus, CI, fast-build config
- [foxtrot](https://github.com/janhohenheim/foxtrot) — 3D "mega-example": curated crate stack, TrenchBroom levels-as-data
- [bevy_new_minimal](https://github.com/TheBevyFlock/bevy_new_minimal) — the irreducible baseline

Feedback loop:
- [Bevy hotpatching design doc](https://hackmd.io/u_Od5TNXShK45m8ii-s4kA) · [hotpatching PR #19309](https://github.com/bevyengine/bevy/pull/19309)
- [bacon](https://dystroy.org/bacon/) — background checker
- [bevy-inspector-egui](https://github.com/jakobhellermann/bevy-inspector-egui) — live world inspection

Testing:
- [bevy_tdd_book](https://github.com/richelbilderbeek/bevy_tdd_book)
- [Automated testing in Bevy](https://chadnauseam.com/coding/gamedev/automated-testing-in-bevy/) · [headless mode guide](https://taintedcoders.com/bevy/how-to/headless-mode)
- [BITT](https://github.com/haihala/Bevy-integration-testing-toolkit) — input playback + screenshot testing (abandoned at Bevy 0.13 — we own the pattern instead, see Remote Interaction Testing)
- [proptest](https://proptest-rs.github.io/proptest/) · [cargo-nextest](https://nexte.st/) · [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)

Tooling & hygiene:
- [bevy_cli / bevy_lint](https://github.com/TheBevyFlock/bevy_cli) · [arctic_tern](https://github.com/TheBevyFlock/arctic_tern) (Bevy migrations)
- [cargo-deny](https://embarkstudios.github.io/cargo-deny/) · [cargo-machete](https://github.com/bnjbvr/cargo-machete)
- [Bevy cheat book: performance pitfalls](https://bevy-cheatbook.github.io/pitfalls/performance.html)

## License

[MIT](LICENSE). Contributions are accepted under the same license.
