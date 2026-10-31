# Mod core: known issues

Problems found while working on the core and left for later, because they don't lose data or stop a mod for good.
Fixed ones are removed from this list.

## Behaviour

- **Spawn and hurt events cover one level.** The other levels' mobs and redstone work (`tick_parked_mod_levels`), but
  `entity-spawned` and `entity-hurt` still only compare the level in place. Their numbers lag a tick there.
- **A mod's mob type shows its template** to every client, and vanilla code comparing `kind.id` with a name won't treat
  it as the template (the species code does).
- **Events after the fact.** `entity-spawned` and `entity-hurt` compare mobs at the end of the tick. A cancel undoes
  the result: a cancelled mob existed for a tick, and knockback stays.
- **Redstone after all mods stop.** `Ticks.mod_outputs` isn't cleared when no server mod runs any more, until the level
  reloads (`tick_server_mods` returns before `sync_mod_redstone`).
- **A window's placeholder** lives in `block_entities` under a key no block has. Not checked: what a save does with one
  while a window is open.
- **In-frame single player** opens no mod windows (`--in-frame` only).
- **Veins stop at chunk edges** (`vanillagen.rs` `place_mod_ores`).
- **Locked results are per process**, so a client's own recipe display in a window beside a server thread follows the
  server's list. A world switch keeps the old list until the new world's mods set theirs.

- **`crafted` events** come from the menus guests use, so not from `--in-frame` play. Shift-clicking a stack reports the
  recipes the menu lists, not how many items came out.
- **Player events are found by comparing** health and dimension at the tick's end: dying and respawning within one tick
  shows neither.

## Performance (not measured)

- World reads are per block and format a string each call: a batch read would help cable networks.
- `block-data.json` holds everything, in hex, written whole at each save.
- `declare` and `schedule-tick` count a mod's entries on every call.
- `neighbor_hooks` compares six neighbours of every mod block on every tick.

## Items

- **Other components don't reach clients.** `lan/guest.rs` `network_stack` sends the ones it knows: name, lore, model
  and shine now, but not food, tool and the rest.

## Dead code

`EventBus` subscribers, `DeferredRegister`, `ConfigManager`, IMC, recipe resolvers, loot and the rest of the methods in
`docs/mod-core-plan.md` section 4 still aren't reachable from a mod. `storage.config` doesn't use `ConfigManager`: it
needed one read at start and no registry.
