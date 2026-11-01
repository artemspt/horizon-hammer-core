# Expansion boards

A sandboxed mod can do only what the mod API offers. When that isn't enough, a developer writes an **expansion board**: a native library, in a
`.hboard` file, offering functions the API doesn't have. Mods call them like any other function.

| | Mod (`.hmod`) | Board (`.hboard`) |
|---|---|---|
| Runs as | WebAssembly, in a sandbox | native code |
| Can do | what the API offers | anything the player can do on the computer |
| Speed limit | a time budget per call | none: it holds the game while it works |
| A crash | stops that mod | **takes the whole game down** |
| Installing | `horizon-mods/`, then turn on | `horizon-boards/`, then turn on, marked unsafe |

Write a board only when a mod can't do the job: hot loops such as a large energy network, or a library you already have in Rust or C. Everything else
belongs in a mod, which players can trust without reading your code.

## For players

The game makes a `horizon-boards` folder beside `horizon-mods`. Put `.hboard` files in it, then:

1. Open **Options > Mods**. Boards, and mods that call them, are red and start with `[UNSAFE]`.
2. Turn the board on, only if you trust its author: **it can do anything you can on this computer**.
3. Restart the game, or type `/hmod full-reload` in a world.

A board that was replaced with another file is off again, so an update asks for your trust again. `/hmod boards` lists boards and what's wrong with
them. A mod needing a board you don't have says which one, and stays off.

## For developers

You need [Rust](https://rustup.rs) and `cargo-horizon-mod`.

```bash
cargo horizon-mod board new my_board
```

```bash
cd my_board && cargo horizon-mod board build
```

The file lands in `target/horizon-mod/my_board.hboard`. A complete example is `mods/example_board`, and `mods/board_user` is a mod calling it.

### The board

`horizon-board.toml`:

```toml
id = "my_board"             # a-z, 0-9, _
name = "My Board"
version = "1.2.0"           # a mod can ask for a major version
description = "Say what it does: a player reads this before trusting it."
authors = ["You"]
functions = ["sum"]         # calls to any other are refused
conflicts = ["other_board"] # optional: can't run beside these
```

`src/lib.rs`:

```rust
use horizon_board_sdk::{export_board, host};

fn sum(input: &[u8]) -> Result<Vec<u8>, String> {
    host::log("summing");
    let total: u64 = input.iter().map(|b| u64::from(*b)).sum();
    Ok(total.to_le_bytes().to_vec())
}

export_board!("sum" => sum);
```

A function takes bytes and gives bytes or a reason: the format is yours. Say it in your board's documentation. A panic comes back to the mod as an error,
unless you build with `panic = "abort"`. A memory fault can't be caught, and ends the game.

During a mod's call, `host` reaches the game, acting for that mod: `host::block`, `host::set_block`, `host::run_command`. `host::log` and `host::warn`
work anywhere. Writes land at the tick's end.

### The mod

```toml
boards = ["my_board@1"]     # or just "my_board"
```

```rust
use horizon_mod_sdk::server::boards;

let answer = boards::call("my_board", "sum", &[1, 2, 3])?;
```

A mod calls only the boards it lists, is marked unsafe, and doesn't start without them. Server mods only for now.

### Other systems

A package holds one library per system: `native/linux-x86_64.so`, `native/windows-x86_64.dll`, `native/macos-aarch64.dylib`. Building replaces this
computer's and keeps the others, so run `cargo horizon-mod board build` on each system with the file from the last one in `target/horizon-mod/`. A player
on a system you skipped sees "not built for this computer".

## Conflicts

Checked at start, before any library opens:

- **Two boards with `conflicts`** between them, in either direction: neither runs, and each names the other.
- **Two files with one id:** neither runs.
- **Version:** `my_board@2` with 1.4.0 on: the mod stays off and says so.
- **Names** can't clash: a function is always called with its board's id.

## What it can't do yet

- **No hooks.** A board answers calls from mods; it isn't told about ticks or events by itself, and the game has no call into it otherwise. Reaching
  further means adding to `abi::Host` and the game, or, once merged from upstream, a native mod patching the game's source.
- **Turning off takes a restart.** A library stays open until the game exits. `/hmod full-reload` is that restart without
  going through the menus.

## `/hmod full-reload`

The game saves and closes the world, starts itself again on the same world, and exits once the new one has its world in
place, a few seconds of a hidden window. Not for a server you joined, nor for a dedicated server.

If the new game crashes or isn't ready in 3 minutes, it is killed and the old one opens the world again, saying why in chat.
So a board that crashes at load doesn't lose you the session.

What doesn't survive: anything unsaved in a mod's memory, open screens, and the LAN game: other players must join again once
you reopen it. Started from the launcher, the launcher's Stop button no longer reaches the game: close its window instead.

Run on Linux only so far; Windows and macOS use the same standard calls but haven't been tried.
- **Client mods** can't call boards.
- Run on Linux only so far. Windows and macOS builds are written and untested.
