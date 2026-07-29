# internal-remotes (placeholder)

Black-box tests that drive the running game from outside the process over BRP
— the build is an artifact, not a library: spawn the binary (`--features
dev`), poll until BRP answers on 127.0.0.1:15702, drive, assert, kill.

Kinds of tests that belong here:

- protocol contract: the game honors `game/*` (TODO: the BRP facade — tests
  speak the contract, never raw component queries)
- acceptance probes: hot reload lands in <2s, box oscillates per shipped config
- game+tool pairs, once something lands in `tools/`: editor connects and edits,
  auth handshake (token accepted / rejected), version negotiation fails loudly

Not a cargo test target yet: add a `main.rs` here to make it one (cargo
provides the binary path via `env!("CARGO_BIN_EXE_bevy-dx")`), mark tests
`#[ignore]` (slow, need a built binary), and add a justfile recipe running
`cargo test --features dev --test internal-remotes -- --ignored`.
