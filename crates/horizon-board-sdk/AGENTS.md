# Writing an expansion board: for an AI agent

Read `docs/boards.md` for the what and why. This page is the rules.

## Decide first

Default to a mod, not a board. A board costs every player a warning and their trust, and can crash the game. Write one only when the mod API can't do the
job, or a sandboxed call is measurably too slow. If a function would serve many mods, propose it for the core instead: `crates/horizon-mods/AGENTS.md`.
Don't tell a user a board is safe, and don't turn one on for them.

## Make one

```bash
cargo horizon-mod board new my_board
```

```bash
cargo horizon-mod board build
```

Files: `horizon-board.toml` (see below), `Cargo.toml` with `crate-type = ["cdylib"]`, `src/lib.rs` ending in `export_board!`. Copy `mods/example_board`.

```toml
id = "my_board"             # required, [a-z0-9_]{1,64}
name = "My Board"           # required
version = "1.2.0"           # required; the first number is what `my_board@1` matches
functions = ["sum"]         # required, at least one
description = "..."         # shown to the player deciding to trust it
authors = ["..."]
conflicts = ["other"]
```

Unknown fields are errors. Every name in `functions` needs a `"name" => function` in `export_board!`, and the other way round: a call to a name not listed
is refused before it reaches the board.

## Contract

- A function: `fn(&[u8]) -> Result<Vec<u8>, String>`. At most 16 MiB back. Document your byte format.
- `horizon_board_sdk::host`: `log`, `warn` anywhere. `block`, `set_block`, `run_command` only inside a call from a mod, on that thread; elsewhere they fail.
- `set_block` and `run_command` land at the tick's end, so reading back in the same call shows the old world.
- A call runs on the game's thread, or the server's, and blocks it: no sleeping, no unbounded loops. Keep long work on your own thread and hand back results in a later call.
- Your statics persist across `/hmod reload`, world changes and mods: one process has one copy. The window and the server thread may both call you, from two threads, so a `static` needs to be `Sync`.
- Never let `panic = "abort"` into a board: unwinding is how a panic becomes an error.

## Changing the interface itself

`crates/horizon-board-sdk/src/lib.rs` `abi` is a C interface boards are built against. Add to the end of `abi::Host`, and to `HOST` in
`crates/horizon-mods/src/runtime/board_calls.rs`. Never reorder or change a field: that breaks built boards. Only raise `ABI` if you must, because it
refuses every board out there. The mod side is the WIT interface `boards`, which follows `crates/horizon-mods/AGENTS.md`.

## Check it

```bash
cargo test -p horizon-mods --features runtime,pack --test boards
```

To try it in the game, put the `.hboard` in `horizon-boards/`, the mod in `horizon-mods/`, and in `horizon-mods.txt`:

```
mod.board.my_board@<first 16 hex digits of the file's SHA-256>:true
```

The hash is the player's consent for that exact file: only write it for a throwaway test folder, never a user's own. Then `/hmod boards`. See "Testing" in
`crates/horizon-mods/AGENTS.md` for running the game safely.
