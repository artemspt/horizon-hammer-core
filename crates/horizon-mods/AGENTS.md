# Working on the mod API

For an agent or developer joining to extend the mod API. Read this page, then take the next unticked items in
[docs/mod-api-plan.md](../../docs/mod-api-plan.md).

## What this is

Horizon is Minecraft: Java Edition 26.3 rewritten in Rust. Upstream (`horizon-rust/horizon`) dropped its WebAssembly mods on
2026-10-29 for native ones, which patch the game's source, have full access and need the game rebuilt. This fork keeps the
sandbox: a mod is a file you drop in, it can do only what its manifest lists, and it reloads while the game runs. The aim is
a NeoForge-like core big enough for Twilight Forest, TerraFirmaCraft or GregTech analogues.

The sandbox's weakness is that a mod can do only what the API offers. **Your job is to grow that API**: most useful first,
real behaviour, no stubs.

## Setting up from nothing

You need Rust stable 1.95 or newer, Python 3.10+, JDK 25, and on Linux `libasound2-dev libudev-dev`. For the game's resources
you need **your own copy** of Minecraft 26.3: play it once in the official launcher, and get the server jar from
`downloads.server.url` in `versions/26.3/26.3.json`.

```bash
rustup target add wasm32-unknown-unknown
```

```bash
cargo run --release -p horizon-prepare -- --prepare --server-jar ~/Downloads/server.jar
```

```bash
cargo build --profile fast -p horizon-client -p cargo-horizon-mod --features horizon-client/mods --bins
```

```bash
./mods/build-all.sh
```

```bash
cargo test -p horizon-mods --features runtime,pack
```

Resources land in the gitignored `.local/horizon-client/26.3`, about 2 GB. **Never commit or share them, or any game file:**
each person prepares their own. Use `--profile fast`: a debug game takes 25 seconds to reach a world, this one 4.

To play with the mods:

```bash
cargo run --profile fast -p horizon-client --features mods -- --resources .local/horizon-client/26.3
```

## Adding a function

Every function touches these, in this order:

1. **Contract:** `crates/horizon-mod-sdk/wit/horizon.wit`, with a doc comment saying what it does and when it fails. The SDK
   re-exports interfaces, so an existing interface needs nothing there; a new one needs its `pub use` in `src/lib.rs`.
2. **Level:** `src/api_level.rs`. Add `("interface.name", N)`, raise the array's length, raise `LEVEL` when starting a new level.
   Keep one level per batch you land. The `api_reference` test fails until this is right.
3. **Host:** the interface's `impl ...::Host for State` in `src/runtime.rs` (`world`, `players`) or `src/runtime/server_api.rs`
   (`entities`, `storages`). Check a mod's input here: a mod is untrusted, and a string reaching a command must not be able to
   end its argument (`check_game_id`, `quoted`).
4. **Game, if it needs one:** `crates/horizon-client/src/game/mod_machines.rs`. Add a `ServerAction` or `MobChange`, handle it in
   `apply_mod_actions`, and in `tick_parked_mod_levels` for other levels. Also `crates/cargo-horizon-mod/src/check.rs`, which prints
   each action. New things to read go through `WorldView` or `Snapshot`.
5. **A hook** is an export: an events interface and its host world in the WIT, a method on the SDK's `ServerMod`, the
   `export_server_mod!` wiring, a name in `manifest::EVENTS` so mods that don't ask pay nothing. See `game-events` in `src/runtime/events.rs`.
6. **Proof:** call it from `api_tour` in `mods/showcase/src/lib.rs`, and check the answer.
7. **Docs:** regenerate the reference with the test command below, tick `docs/mod-api-plan.md`, add to the changelog in the root `AGENTS.md`.

Prefer the cheapest way that really works: compose existing reads, then a vanilla command via `self.console`, then game code.

## Testing

```bash
HORIZON_WRITE_DOCS=1 cargo test -p horizon-mods --features runtime,pack
```

That proves little about the game: **run it**. Use a throwaway folder and your own socket, never the default one, and keep away from a
game someone is playing. `horizon-ctl help` lists its commands.

```bash
export HORIZON_CONTROL=/tmp/my-test.sock HORIZON_NO_AUDIO=1
```

```bash
T=$(mktemp -d) && mkdir $T/horizon-mods && cp target/horizon-mod/showcase.hmod $T/horizon-mods/ && echo 'mod.showcase:true' > $T/horizon-mods.txt
```

```bash
R=$PWD; cd $T && $R/target/fast/horizon-ctl launch --game $R/target/fast/horizon-client --world Test --seed 5 --creative --wait --render-distance 4 --resources $R/.local/horizon-client/26.3
```

Then `horizon-ctl chat "/showcase api set"`, `horizon-ctl chat "/showcase api check"`, `horizon-ctl screenshot out.png`, `horizon-ctl state`,
`horizon-ctl block X Y Z`, `horizon-ctl quit`. On a machine with no screen, start `Xvfb` and set `DISPLAY`.

Say plainly what you ran and what you didn't. A command that was accepted isn't a command that worked: mods don't see a console
command's failures, so look at the result in the game.

## What bites

- **Calls happen with the world lent.** During a mod's call the game's world is out of its usual place, so reads during a hook go
  through `WorldView`. Code flying a projectile before `lend_world` saw air everywhere until it was moved.
- **The hub can be out of `self.lan`** during a guest's command: pass it along, or output and reads go missing.
- **Changes wait.** Actions run at the tick's end, so a read right after a write sees the old world. Tests take two commands (`api set`, then `api check`).
- **Several levels.** Mobs and reads may be of the level in place only. Say so in the doc comment when true, and try a parked level.
- **Selectors don't take a player's UUID:** `console` swaps ids for names, so build lines with `self.console`.
- **Mod dimensions** are `Dim::Mod`: use `dim.base()` for what a vanilla type decides, or they fall through a match.
- **Limits:** a `State` queues at most `ACTIONS_PER_CALL`; a call has a time budget, so bound every loop a mod can feed.
- **Level tests:** a test pinning an example to `LEVEL` fails when you raise it. Pin the number.

## Ground rules

- Stay in `crates/horizon-mods/`, `crates/horizon-mod-sdk/`, `crates/cargo-horizon-mod/`, `mods/`. Touch the game only to integrate, in as few lines as you can, and put new
  game code in the `mod_*.rs` files: every line in upstream's files is a conflict at the next merge. See [docs/fork.md](../../docs/fork.md).
- Work on your own branch and land small batches, 30 to 75 functions, often. Someone else edits the same files: pull often, and don't reformat what you didn't change.
- No stubs, no functions returning a fixed answer. Can't be done in the sandbox? Say so in the plan and move on.
- Don't commit `.local/`, `.idea/workspace.xml`, `.serena/`, `options.txt`, `horizon-mods/`, or saves.
- English in code, comments and commits. Comments say why, in the style around them.
- Don't push to `main`, or push anything to upstream.
