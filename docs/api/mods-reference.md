<!-- Generated from crates/horizon-mod-sdk/wit/horizon.wit and the manifest's capabilities by crates/horizon-mods/tests/api_reference.rs; a test fails when this file is out of date. Update it with: HORIZON_WRITE_DOCS=1 cargo test -p horizon-mods --features pack --test api_reference -->
# Mod API reference

The contract between Horizon and its mods: the WIT package `horizon:mods@0.1.0`, every interface, type, function and hook in it, and the capabilities a manifest may list. The guide is [mods.md](../mods.md); stability and versions: [stability.md](stability.md). Manifest `api` lines this game serves: `0.1`, `0.2`, `0.3`.

## Capabilities

| In `capabilities` | Grants | The player reads "May …" |
| --- | --- | --- |
| `game-read` | [`game`](#game) | see your position, health and the time |
| `chat` | [`chat`](#chat) | show you chat messages |
| `hud` | [`hud`](#hud) | draw text on your screen |
| `storage` | [`storage`](#storage) | save its own settings to disk |
| `command` | [`command`](#command) | run commands as you, with your permissions |
| `registries` | data files (`assets/`, `data/`), no interface | add blocks and items to your worlds |
| `ui` | [`ui`](#ui) | show its own screens and a settings page |
| `render` | [`render`](#render) | draw on your screen, shake and zoom the camera |
| `input` | [`input`](#input) | see the keys you hold and your clicks, take over the use button on its own items and the mouse wheel, and add keys |
| `players` | [`players`](#players) | see who is on the server, message them and change their inventories |
| `world` | [`world`](#world) | read and change the world's blocks, and keep items, energy and fluids on them |
| `recipes` | [`recipes`](#recipes) | read the game's recipes and items and which recipes you know, place recipes in your crafting grid and, in creative mode, give you items |
| `entities` | [`entities`](#entities) | find, spawn, hurt and move mobs and players, give effects, show particles and play sounds |
| `net` | [`client-net`](#client-net) | send messages between your game and the server's mods |
| `worldgen` | [`worldgen`](#worldgen) | modify world generation surface rules |

## World `events-host`

The game's side of `game-events`.

- Imports (what the game offers): `players`, `entities`
- Exports (what the mod provides): `game-events`

## World `before-events-host`

The game's side of `before-events`.

- Imports (what the game offers): `players`, `entities`
- Exports (what the mod provides): `before-events`

## World `entity-events-host`

- Imports (what the game offers): `players`, `entities`
- Exports (what the mod provides): `entity-events`

## World `logic-events-host`

- Imports (what the game offers): `players`, `entities`
- Exports (what the mod provides): `logic-events`

## World `client-net-host`

The game's side of `client-net-events`.

- Imports (what the game offers): 
- Exports (what the mod provides): `client-net-events`

## World `server-net-host`

The game's side of `server-net-events`.

- Imports (what the game offers): `players`
- Exports (what the mod provides): `server-net-events`

## World `horizon-mod`

A client mod: runs in the player's game.

- Imports (what the game offers): `log`, `game`, `chat`, `hud`, `storage`, `command`, `client-net`
- Exports (what the mod provides): `events`

## World `block-host`

A server mod (manifest `side = "server"`, `api = "0.3"`, experimental): runs where the world is simulated, on the dedicated server, a LAN host and single player's own server. `block-events` for the server: a world of its own, so a server mod built before it still instantiates as `horizon-server-mod` and is not asked.

- Imports (what the game offers): `players`
- Exports (what the mod provides): `block-events`

## World `horizon-server-mod`

- Imports (what the game offers): `log`, `storage`, `players`, `world`, `storages`, `entities`, `worldgen`, `server-net`
- Exports (what the mod provides): `server-events`

## World `horizon-server-block-mod`

What the SDK builds a server mod as: `horizon-server-mod` and the block hooks.

- Imports (what the game offers): `log`, `storage`, `players`, `world`, `storages`, `entities`, `worldgen`, `server-net`
- Exports (what the mod provides): `block-events`, `game-events`, `before-events`, `entity-events`, `logic-events`, `server-net-events`, `server-events`

## World `horizon-ui-mod`

A mod with widgets (capability `ui`): the SDK builds every mod against this world. Mods built for `horizon-mod` alone (no `ui-events` export) still load and simply hear nothing of their widgets.

- Imports (what the game offers): `ui`, `render`, `input`, `recipes`, `browser`, `log`, `game`, `chat`, `hud`, `storage`, `command`, `client-net`
- Exports (what the mod provides): `ui-events`, `client-net-events`, `events`

## World `ui-host`

The game's side of `horizon-ui-mod` beyond `horizon-mod`.

- Imports (what the game offers): `ui`
- Exports (what the mod provides): `ui-events`

## World `render-host`

The game's side of capability `render` (imports only).

- Imports (what the game offers): `render`, `input`
- Exports (what the mod provides): 

## World `recipes-host`

The game's side of capability `recipes` (imports only).

- Imports (what the game offers): `recipes`, `browser`
- Exports (what the mod provides): 

## `log`

Access: always granted.

The mod's log, in the game's log under the mod's id. Always granted.

### enum `level`

| Case | |
| --- | --- |
| `debug` |  |
| `info` |  |
| `warn` |  |
| `error` |  |

| Function | Signature | What it does |
| --- | --- | --- |
| `write` | `func(level: level, message: string)` | One line in the game's log as `[mod <id>] message`, at that level. |

## `game`

Access: capability `game-read`.

Capability `game-read`: the local player, the world clock and the block the player looks at.

### record `player`

| Field | Type | |
| --- | --- | --- |
| `x` | `f64` | Feet position, in blocks. |
| `y` | `f64` |  |
| `z` | `f64` |  |
| `yaw` | `f32` | Degrees, as F3 shows them. |
| `pitch` | `f32` | Degrees: -90 looks straight up, 90 straight down. |
| `health` | `f32` | Hit points as vanilla counts them: 20 is full health without effects. |
| `food` | `u32` | 0 to 20, as the hunger bar. |
| `dimension` | `string` | `minecraft:overworld`, `minecraft:the_nether`, ... |

### record `clock`

| Field | Type | |
| --- | --- | --- |
| `game-time` | `u64` | Ticks since the world was created. |
| `day-time` | `u64` | 24000 ticks per day; 0 is 6:00 on the first day. |

### record `hit`

A block along the player's view.

| Field | Type | |
| --- | --- | --- |
| `block-x` | `s32` | The block hit. |
| `block-y` | `s32` |  |
| `block-z` | `s32` |  |
| `x` | `f64` | The point on its face where the view meets it. |
| `y` | `f64` |  |
| `z` | `f64` |  |
| `block` | `string` | `minecraft:stone`, ... |

| Function | Signature | What it does |
| --- | --- | --- |
| `get-player` | `func() -> option<player>` | None on the title screen. |
| `get-clock` | `func() -> option<clock>` | None on the title screen. |
| `raycast` | `func(distance: f32) -> option<hit>` | The first block along the player's view (fluids pass), up to `distance` blocks (at most 1024; past the loaded chunks, across the distant terrain when LOD Distance is on), as of this tick. None when nothing is hit, and on the title screen. |

## `chat`

Access: capability `chat`.

Capability `chat`: messages only the local player sees.

| Function | Signature | What it does |
| --- | --- | --- |
| `show` | `func(text: string)` | Shown in chat after the mod's name, like a system message. |

## `hud`

Access: capability `hud`.

Capability `hud`: one panel of text lines per mod, drawn by the game every frame until the mod changes or clears it (no call per frame).

### enum `corner`

| Case | |
| --- | --- |
| `top-left` |  |
| `top-right` |  |
| `bottom-left` |  |
| `bottom-right` |  |

### record `line`

| Field | Type | |
| --- | --- | --- |
| `text` | `string` | Plain text, up to 256 characters. |
| `color` | `u32` | 0xRRGGBB. |

| Function | Signature | What it does |
| --- | --- | --- |
| `set-panel` | `func(corner: corner, lines: list<line>)` | Replaces the mod's panel: up to 16 lines in that corner of the screen. |
| `clear` | `func()` | Removes the mod's panel. |

## `storage`

Access: capability `storage`.

Capability `storage`: a per-mod folder of small key/value files, quota'd (docs/mods.md). Keys are `[a-z0-9_.-]{1,64}`, one path segment.

| Function | Signature | What it does |
| --- | --- | --- |
| `get` | `func(key: string) -> option<list<u8>>` | None if `key` has never been `set`, or the key is not well-formed. |
| `set` | `func(key: string, value: list<u8>) -> result<_, string>` | Errs on a malformed key or over quota (bytes for the whole mod). |
| `delete` | `func(key: string)` | No error if `key` was already absent. |
| `config` | `func(key: string) -> option<string>` | A setting the player can edit: `horizon-mods/config/<mod id>.toml`, made on first run from the manifest's `[config]` table, which also fills what the file leaves out. Read when the mod starts: `/hmod reload` reads it again. None for a key in neither. |
| `read-resource` | `func(path: string) -> option<list<u8>>` | A file of the mod's own package: `data/<mod id>/machines.json`, the data a NeoForge mod loads with a reload listener. Only under `assets/` or `data/`, in a mod with capability `registries`. None when absent. |

## `command`

Access: capability `command`.

Capability `command`: vanilla commands run as the player, with the player's permissions, after the hook call returns (8 per call). A mod's own commands are not run this way.

| Function | Signature | What it does |
| --- | --- | --- |
| `run` | `func(line: string)` | `line` without its slash; its feedback goes to the chat as if typed. |
| `run-quiet` | `func(lines: list<string>)` | `lines` (each without its slash, at most 256 a call) as `run` does, but with nothing in the chat: no feedback and no failure. In single player and on Horizon's servers, each runs exactly as the player typing it would (the same rights), in order, 64 a tick, and the server logs one line a batch; one that carries a chat message (`say`, `msg`) is refused. On any other server they go as chat commands, and there their feedback shows: a mod running many should say so. |

## `ui`

Access: capability `ui`.

Capability `ui`: widgets the game draws with its own menus' widgets, for a screen the mod owns and a settings page in Options > Mods. Layout and drawing are the game's: a mod says what its widgets hold, never where they sit or how they look, and the game clips everything it is handed.

### record `text-widget`

A line of text.

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` | The mod's own name for the widget; `ui-events` calls echo it. |
| `text` | `string` | Up to 256 characters. |
| `color` | `u32` | 0xRRGGBB; 0 is the menus' white. |

### record `item-widget`

An item's icon and a name beside it, as recipe viewers show them.

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` | The mod's own name for the widget. |
| `item` | `string` | `minecraft:diamond`, `<mod>:<path>`; an unknown id draws a "?". |
| `name` | `string` | The name shown beside the icon (empty: the item's own). |

### record `list-widget`

Rows the player picks one of, as an options page lists choices: a pick is `ui-events::select`, `selected` is the row to mark now.

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` | The mod's own name for the widget; `ui-events::select` echoes it. |
| `rows` | `list<string>` | The rows' texts, at most 512, each up to 256 characters. |
| `selected` | `option<u32>` | The row to mark as the current choice, if any. |

### record `button-widget`

A thing on a mod's screen or settings page. A button as the options pages have them (200 × 20): a click is `ui-events::select(id, 0)`. Mods cycle its label for a choice ("Corner: Top Left").

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` | The mod's own name for the widget. |
| `label` | `string` | Up to 256 characters. |

### variant `widget`

| Case | Payload | |
| --- | --- | --- |
| `text` | `text-widget` | Text the player reads. |
| `button` | `button-widget` | A button. |
| `item` | `item-widget` | An item's icon and name. |
| `list` | `list-widget` | Rows the player picks from. |

### record `screen`

A screen the mod owns: a title over widgets stacked down the middle, the Done button the game adds, and Esc to leave it.

| Field | Type | |
| --- | --- | --- |
| `title` | `string` | Its title, up to 256 characters. |
| `widgets` | `list<widget>` | Its widgets, in order, at most 64. |

| Function | Signature | What it does |
| --- | --- | --- |
| `open-screen` | `func(screen: screen)` | Show `screen` over the game; a second call while it is open replaces it. Only the last call of one hook counts. |
| `close-screen` | `func()` | Close the mod's screen when it is open (the mod hears no `closed` for this: it was its own doing). |
| `set-settings` | `func(widgets: list<widget>)` | The mod's page in Options > Mods, reached from a settings button on its row: the widgets it shows, an empty list for no page. The player opening the page is `ui-events::settings-opened`: push fresh widgets then; later calls update the page while it shows. |
| `replace-screen` | `func(vanilla: string, replacement: option<screen>) -> result<_, string>` | Show the mod's own screen in place of a vanilla one: from now on, when the player would open `vanilla` (`minecraft:inventory`: the inventory key, in survival and creative alike), the game opens `replacement` as the mod's screen instead, not drawn over it. While it shows, `open-screen` updates it and leaving it is `ui-events::closed` as for any of the mod's screens; a later call replaces it, and none gives the vanilla screen back. The first mod (in load order) that replaces a screen gets it. Err for a screen this Horizon can't replace. |

## `ui-events`

Access: exported by the mod: the game calls it.

What the game calls on a mod that shows widgets (capability `ui`): what the player did with them.

| Function | Signature | What it does |
| --- | --- | --- |
| `select` | `func(id: u32, row: u32)` | A row of one of the mod's `list` widgets was picked: the widget's `id` and the row, on the screen or the settings page that showed it. |
| `closed` | `func()` | The player left the mod's screen (Esc or Done); not for a screen the mod closed itself or one that went away with the mod. |
| `settings-opened` | `func()` | The player opened the mod's settings page in Options > Mods: call `ui::set-settings` now to fill it with fresh widgets. |

## `input`

Access: capability `input`.

Capability `input` (experimental: the manifest needs `experimental = ["input"]`): the use button on items the mod claims, and the mod's own keys. Events wait for `poll`, at most 256.

### record `held-item`

The main hand's stack.

| Field | Type | |
| --- | --- | --- |
| `id` | `string` | `minecraft:spyglass`, `<mod>:<path>`. |
| `count` | `u32` |  |
| `slot` | `u32` | The hotbar slot, 0 to 8. |

### record `item-use`

| Field | Type | |
| --- | --- | --- |
| `item` | `held-item` |  |
| `ticks` | `u32` | Game ticks the button was held (0 on `use-start`). |

### variant `event`

| Case | Payload | |
| --- | --- | --- |
| `use-start` | `item-use` | The use button went down with a claimed item in the main hand. The game does nothing else with the click: no placing, no eating, no swing. |
| `use-stop` | `item-use` | It came up again, or the item left the hand, or a screen opened. |
| `key-down` | `string` | A key `register-key` named went down or up while playing (no screen open). The game's own mapping on that key still runs. A screen opening lets every held key up. |
| `key-up` | `string` |  |
| `wheel` | `f32` | Mouse wheel steps while the mod holds the wheel (`claim-wheel`): positive away from the player, as the hotbar reads them. |

### record `mapping-state`

A mapping as the player has bound it.

| Field | Type | |
| --- | --- | --- |
| `label` | `string` | The bound key's name as Controls shows it: `W`, `Space`, `LMB`. |
| `down` | `bool` | Held down now (as of this frame for a mod with a `frame` hook, else this tick). |

| Function | Signature | What it does |
| --- | --- | --- |
| `claim-items` | `func(ids: list<string>)` | The items whose use the mod handles instead of the game, by id (at most 64). Replaces the previous list. |
| `register-key` | `func(name: string, label: string, key: string) -> result<_, string>` | A key of the mod's: `name` comes back in `key-down` and `key-up`, `label` says what it does, `key` is the key as options.txt writes it (`key.keyboard.r`), the default: the player can bind another one on the mod's page in Options > Mods, and that one wins. At most 16; an unknown key is an error. |
| `held` | `func() -> option<held-item>` | The main hand's stack, as of this tick; none when it is empty. |
| `poll` | `func() -> list<event>` | The events since the last call, oldest first. |
| `claim-wheel` | `func(on: bool)` | Takes the mouse wheel while playing (true) or gives it back: while taken, its steps are the mod's `wheel` events and the hotbar doesn't move. The game gives it back when the mod stops. |
| `mapping` | `func(name: string) -> option<mapping-state>` | A movement or click mapping as the player has bound it: `key.forward`, `key.left`, `key.back`, `key.right`, `key.jump`, `key.sneak`, `key.sprint`, `key.attack` or `key.use`. None for another name. |
| `clicks-per-second` | `func() -> tuple<u32, u32>` | Clicks of the attack and use buttons in the last second. |

## `render`

Access: capability `render`.

Capability `render` (experimental: the manifest needs `experimental = ["render"]`): shapes the game draws for the mod with its own renderer, kept until the mod replaces them (no call per frame). A mod never supplies shader code.

### enum `corner`

A corner of the screen, as `hud`'s panels use them.

| Case | |
| --- | --- |
| `top-left` |  |
| `top-right` |  |
| `bottom-left` |  |
| `bottom-right` |  |

### record `rect`

A rectangle in GUI pixels: the top left corner and the size.

| Field | Type | |
| --- | --- | --- |
| `x` | `f32` |  |
| `y` | `f32` |  |
| `w` | `f32` |  |
| `h` | `f32` |  |

### record `point`

A point in GUI pixels from the screen's top left.

| Field | Type | |
| --- | --- | --- |
| `x` | `f32` |  |
| `y` | `f32` |  |

### record `fill-shape`

| Field | Type | |
| --- | --- | --- |
| `rect` | `rect` |  |
| `color` | `u32` | 0xAARRGGBB. |

### record `quad-shape`

Any four-cornered shape (a wheel's segment, an arrow): the corners in order round it.

| Field | Type | |
| --- | --- | --- |
| `corners` | `tuple<point, point, point, point>` |  |
| `color` | `u32` | 0xAARRGGBB. |

### record `sprite-shape`

| Field | Type | |
| --- | --- | --- |
| `sprite` | `string` | One of the game's GUI sprites, as `gui/sprites/` names them (`hud/heart/full`, `widget/button`), the resource packs' versions included. An unknown one draws nothing. |
| `rect` | `rect` |  |
| `color` | `u32` | A tint, 0xAARRGGBB: 0xFFFFFFFF draws it as it is. |

### record `item-shape`

| Field | Type | |
| --- | --- | --- |
| `item` | `string` | `minecraft:diamond`, `<mod>:<path>`; an unknown one draws nothing. |
| `at` | `point` | The icon's top left. |
| `scale` | `f32` | 1.0 is the hotbar's 16 × 16. |

### record `text-shape`

| Field | Type | |
| --- | --- | --- |
| `text` | `string` | Up to 256 characters, one line. |
| `at` | `point` |  |
| `color` | `u32` | 0xRRGGBB, as `hud`'s lines. |
| `centered` | `bool` | Centred on `at.x` instead of starting there. |
| `shadow` | `bool` |  |

### variant `shape`

| Case | Payload | |
| --- | --- | --- |
| `fill` | `fill-shape` |  |
| `quad` | `quad-shape` |  |
| `sprite` | `sprite-shape` |  |
| `item` | `item-shape` |  |
| `text` | `text-shape` |  |

### record `part-pose`

One part of a placed model, moved from where the file puts it.

| Field | Type | |
| --- | --- | --- |
| `part` | `string` | As in the model's `parts`. |
| `x` | `f32` | Pixels added to its position. |
| `y` | `f32` |  |
| `z` | `f32` |  |
| `pitch` | `f32` | Degrees added to its rotation. |
| `yaw` | `f32` |  |
| `roll` | `f32` |  |

### record `placed-model`

A model of `data/<mod>/horizon/model/<name>.json` standing in the world.

| Field | Type | |
| --- | --- | --- |
| `model` | `string` | `<mod>:<name>`; an unknown one draws nothing. |
| `dimension` | `string` |  |
| `x` | `f64` |  |
| `y` | `f64` |  |
| `z` | `f64` |  |
| `yaw` | `f32` | Degrees. |
| `pitch` | `f32` |  |
| `roll` | `f32` |  |
| `spin` | `f32` | Degrees of yaw a second, turned smoothly by the game. |
| `scale` | `f32` | 1.0 is the model's own size. |
| `parts` | `list<part-pose>` | Its parts' poses now; a part's `spin` in the file turns by itself. |

### record `menu-layout`

How the windows titled `title` look: real menus (`storages.open-menu`), their slots, cursor and shift-clicks all the server's, laid out by the mod.

| Field | Type | |
| --- | --- | --- |
| `title` | `string` |  |
| `width` | `f32` | The window's size in GUI pixels. |
| `height` | `f32` |  |
| `slots` | `list<point>` | Where each of the storage's slots goes, in order; one not listed is hidden. |
| `inventory` | `point` | The top left of the player's inventory, the hotbar 58 below it. |
| `shapes` | `list<shape>` | Drawn under the slots, from the window's top left. |

### record `canvas-area`

A place on a canvas that hears clicks: `ui-events::select(id, 0)`.

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` |  |
| `rect` | `rect` |  |

### record `frame-rate`

| Field | Type | |
| --- | --- | --- |
| `fps` | `u32` | Frames drawn in the last second. |
| `min` | `u32` | The slowest frame's rate, and the 1% low (the 99th percentile frame time's rate), over the last 5 seconds. |
| `low` | `u32` |  |

### record `zoom`

| Field | Type | |
| --- | --- | --- |
| `factor` | `f32` | The field of view is divided by it: 1 is none, at most 10. |
| `smooth` | `bool` | Ease in and out through the game's field-of-view easing, as the spyglass does, instead of at once. |
| `turn` | `f32` | Mouse turning is multiplied by it while zoomed: 1 keeps the player's sensitivity, at least 0.05. |

### record `minimap`

| Field | Type | |
| --- | --- | --- |
| `corner` | `corner` |  |
| `size` | `f32` | Its side in GUI pixels, 16 to 256. |
| `blocks-per-pixel` | `f32` | World blocks per map pixel, 0.5 to 4; the map is 128 × 128 pixels. |
| `rotate` | `bool` | The map turns with the player, who faces up, and an N on its rim marks north. Otherwise north is up and the player's arrow turns. |
| `round` | `bool` | Round, the corners left clear; otherwise square. |

### record `far-terrain`

| Field | Type | |
| --- | --- | --- |
| `distance` | `u32` | How far the distant terrain reaches, in chunks: 64 to 1024. On integrated graphics other than Apple's the game caps it lower. |
| `reduced` | `bool` | Coarser far tiles, as the Far Detail setting's Reduced: 2 to 4 ms less GPU time per frame on integrated graphics. |

| Function | Signature | What it does |
| --- | --- | --- |
| `set-models` | `func(models: list<placed-model>)` | Replaces the models the mod has in the world, up to 1024: drawn every frame until replaced, the way a block entity renderer draws a machine's moving parts. An empty list removes them. |
| `set-menu-layouts` | `func(layouts: list<menu-layout>)` | Replaces the mod's window layouts. |
| `open-canvas` | `func(title: string, shapes: list<shape>, areas: list<canvas-area>)` | Opens or updates a screen drawn entirely by the mod (it needs capability `ui` too): `shapes` in GUI pixels, up to 1024, in any layout, a machine's slots and gauges among them. It leaves as `ui.open-screen`'s does. |
| `set-hud` | `func(shapes: list<shape>)` | Replaces the mod's HUD shapes: up to 1024, drawn every frame in order over the HUD (after the hotbar, under chat and screens), hidden with the HUD (F1). An empty list removes them. |
| `gui-size` | `func() -> tuple<f32, f32>` | The screen's size in GUI pixels (width, height), as of this tick. |
| `shake` | `func(strength: f32, seconds: f32)` | Shakes the camera: a jolt of `strength` (0 to 1, 1 the hardest) that fades out over `seconds` (at most 10). Jolts add up to 1. The view turns, the player doesn't, and the player's Distortion Effects option scales it (0 turns it off). |
| `set-corner` | `func(corner: corner, width: f32, height: f32, shapes: list<shape>)` | Replaces the mod's corner panel: shapes in GUI pixels from the top left of a `width` × `height` box that the game places in `corner`, stacked with the game's overlays and the other mods' panels (under the status effects at the top right), as `hud`'s panels are. |
| `clear-corner` | `func()` | Removes the mod's corner panel. |
| `text-width` | `func(text: string) -> f32` | The width of one line of text in GUI pixels, in the HUD's font: exact for Latin letters (U+0000 to U+024F), 6 per other character. |
| `frame-stats` | `func() -> frame-rate` | The game's frame counter, recomputed twice a second. |
| `set-zoom` | `func(zoom: zoom)` | Zooms the camera until the next call (`factor` 1 ends it), only while playing: a screen opening or the spyglass's own view (which wins) leave the camera vanilla's. With several mods, the strongest zoom counts. |
| `set-minimap` | `func(minimap: option<minimap>)` | Shows a map of the world around the player in `corner`, stacked with the game's overlays and the mods' panels, until the next call (none hides it). It looks like a map item: vanilla's map colours with their height shading and water depth, and the player's arrow. Underground, under a roof and in the Nether it maps the floor around the player's height. The game keeps the map's tiles and refreshes them within its own frame budget; hidden, they are dropped. One map shows at a time: the first loaded mod's. |
| `set-far-terrain` | `func(far-terrain: option<far-terrain>)` | Draws the world beyond the render distance as distant terrain, out to `distance` chunks, until the next call (none ends it). The game builds it on its own workers, from the chunks already loaded or saved and, where nobody has been, from the world's generator; the simulation distance doesn't change. Only worlds whose generator the game has (single player and a LAN world you host) show it. The player's own LOD Distance setting, when on, wins. One far terrain shows at a time: the first loaded mod's. |

## `events`

Access: exported by the mod: the game calls it.

What the game calls. Only the hooks `init` asks for are ever called.

### record `registration`

| Field | Type | |
| --- | --- | --- |
| `commands` | `list<string>` | Chat commands, without the slash. A name vanilla already uses is refused (vanilla's command runs). |
| `tick` | `bool` | Call `tick` every game tick (20 per second while playing). |
| `frame` | `bool` | Call `frame` every rendered frame. Budget: 0.1 ms. |
| `chat` | `bool` | Call `chat` for each chat line the player receives. |

| Function | Signature | What it does |
| --- | --- | --- |
| `init` | `func() -> result<registration, string>` | Once, after loading. An error keeps the mod off and is shown. |
| `tick` | `func(tick: u64)` | Each game tick while in a world, with the game's tick counter. |
| `frame` | `func(seconds: f32)` | Each rendered frame, with the seconds since the last one. |
| `command` | `func(name: string, args: string) -> result<option<string>, string>` | `/name args`: ok(some(text)) is shown as feedback, err(text) in red. |
| `chat` | `func(text: string)` | Each chat line the player receives, as plain text. |

## `recipes`

Access: capability `recipes`.

Capability `recipes` (experimental): the recipes and items the game knows, for recipe viewers. In single player and on Horizon's servers they are the world's; on any other server they are vanilla 26.3's (a client is sent no others), while `known` follows that server's book.

### record `stack`

An item of `items`, by its index there, with a count and the components it has beyond the item's own, in vanilla's item syntax (`[potion_contents={potion:"minecraft:swiftness"}]`), or "".

| Field | Type | |
| --- | --- | --- |
| `item` | `u32` |  |
| `count` | `u32` |  |
| `components` | `string` |  |

### enum `kind`

The station a recipe is made at.

| Case | |
| --- | --- |
| `crafting-shaped` |  |
| `crafting-shapeless` |  |
| `smelting` |  |
| `blasting` |  |
| `smoking` |  |
| `campfire` |  |
| `stonecutting` |  |
| `smithing` |  |
| `brewing` |  |

### record `recipe`

| Field | Type | |
| --- | --- | --- |
| `id` | `string` | `minecraft:oak_planks`. |
| `kind` | `kind` |  |
| `category` | `string` | The recipe book's tab (`crafting_building_blocks`, `furnace_food`), or "". |
| `group` | `string` | Recipes the book shows on one button share a group, or "". |
| `width` | `u32` | A shaped recipe's grid: `slots` holds its cells row by row. 0 for the other kinds. |
| `height` | `u32` |  |
| `slots` | `list<list<stack>>` | Each slot's stacks, any one of which fits; an empty list is an empty cell. Cooking and stonecutting: the input. Smithing: the template, the base, the addition. Brewing: the bottle, the reagent. |
| `results` | `list<stack>` | What it makes; several when that varies (a trim on any armour). |
| `cooking-time` | `u32` | Cooking: game ticks and experience; 0 for the other kinds. |
| `experience` | `f32` |  |
| `book-id` | `s32` | The recipe book's id for it (one per way the book shows it), as `known` lists them; -1 for the kinds the book doesn't show (campfire, stonecutting, smithing, brewing). |

### record `item`

| Field | Type | |
| --- | --- | --- |
| `id` | `string` | `minecraft:diamond`, `<mod>:<path>`. |
| `name` | `string` | In the player's language. |
| `tags` | `list<string>` | Its item tags, without the `#`. |

| Function | Signature | What it does |
| --- | --- | --- |
| `items` | `func() -> list<item>` | Every item, in the registry's order: `stack.item` indexes this list. |
| `all` | `func() -> list<recipe>` | Every recipe: the book's in its order, then campfire, stonecutting, smithing and brewing. They don't change while a world is open. |
| `known` | `func() -> list<s32>` | The `book-id`s of the recipes the player's recipe book holds now. |
| `place` | `func(book-id: s32, all: bool) -> result<_, string>` | Places a recipe in the open screen's grid as a click in the recipe book does (`ServerboundPlaceRecipePacket`): the inventory's grid or a crafting table. `all` places as many as the items allow, as with Shift. Refused while no such screen is open, for a recipe the player's book doesn't hold and for one that can't be placed; one missing items shows as a ghost in the grid, as in the book. |
| `give` | `func(stack: stack) -> result<_, string>` | Creative mode only: gives the player the stack, as taking it from the creative inventory does (`ServerboundSetCreativeModeSlot`): the selected slot first, then the hotbar, then the inventory. The count is cut to one stack of the item (64, 16 or 1). |

## `browser`

Access: not granted by any capability.

Capability `ui` (experimental): a recipe viewer's panel beside the game's inventory and container screens (not the creative inventory), on the right of the screen. The game draws it with its own widgets and slots, pages it and keeps its search box; the mod says what it holds. The first loaded mod with a panel shows.

### record `stack`

An item to show: its id, a count (1 draws none) and its components beyond the item's own, in vanilla's item syntax, or "".

| Field | Type | |
| --- | --- | --- |
| `item` | `string` |  |
| `count` | `u32` |  |
| `components` | `string` |  |

### record `click`

A click on an item of the panel, by its index in `set-items`'s list.

| Field | Type | |
| --- | --- | --- |
| `index` | `u32` |  |
| `right` | `bool` |  |

### record `item-key`

A key the mod registered (`input.register-key`) went down while the pointer was on an item: a slot of the screen's, or the panel's.

| Field | Type | |
| --- | --- | --- |
| `key` | `string` |  |
| `stack` | `stack` |  |

### record `slot-click`

A click on a slot of a recipe the panel shows (`set-recipes`).

| Field | Type | |
| --- | --- | --- |
| `recipe` | `u32` | The recipe's index in `set-recipes`'s list and the slot's in it; the results come after the slots. |
| `slot` | `u32` |  |
| `stack` | `stack` | The stack the slot showed at that moment. |
| `right` | `bool` |  |

### variant `event`

| Case | Payload | |
| --- | --- | --- |
| `clicked` | `click` |  |
| `search` | `string` | The search box's text, each time the player changes it. |
| `key` | `item-key` |  |
| `slot-clicked` | `slot-click` |  |
| `action` | `u32` | The action button of the recipe at this index in `set-recipes`'s list. |
| `page` | `u32` | The player turned the recipes' page (`set-recipes`): the index of the first recipe now shown. |
| `back` |  | The back button of `set-recipes`. |
| `tab` | `u32` | The player picked the tab at this index of `set-recipes`' tabs. |

### enum `layout`

How the game lays out a recipe of `set-recipes`.

| Case | |
| --- | --- |
| `grid` | A crafting grid of `width` x `height` slots, an arrow and the result. |
| `cooking` | One input over a flame, an arrow and the result (furnaces, smokers, campfires). |
| `single` | One input, an arrow and the result (the stonecutter). |
| `smithing` | Template, base and addition, an arrow and the result. |
| `brewing` | A potion and an ingredient, an arrow and the result. |

### record `recipe-view`

A recipe to show. Each slot, and the result, cycles through its stacks, one a second; an empty slot shows nothing.

| Field | Type | |
| --- | --- | --- |
| `layout` | `layout` |  |
| `width` | `u32` | The grid's size (`grid` only), at most 3 x 3. |
| `height` | `u32` |  |
| `slots` | `list<list<stack>>` |  |
| `results` | `list<stack>` |  |
| `note` | `string` | A short line under the recipe (a cooking time), or "". |
| `action` | `option<bool>` | The action button beside it: none, enabled, or shown greyed. |

| Function | Signature | What it does |
| --- | --- | --- |
| `set-items` | `func(items: list<stack>)` | The panel's items, at most 8192 (an empty list hides the panel). |
| `set-recipes` | `func(title: string, tabs: list<stack>, tab: u32, recipes: list<recipe-view>)` | Shows these recipes in the panel instead of its items, as many as fit a page, under `title` and a back button, with a row of `tabs` (each an icon; `tab` is the selected one) between them; an empty list of recipes shows the items again. At most 256 recipes and 16 tabs. |
| `poll` | `func() -> list<event>` | What the player did on the panel since the last poll, at most 256 kept. |

## `players`

Access: capability `players`.

Capability `players`, server mods only (manifest `api = "0.3"`, experimental): the players on the server, and messages to them.

### enum `game-mode`

| Case | |
| --- | --- |
| `survival` |  |
| `creative` |  |
| `adventure` |  |
| `spectator` |  |

### record `player`

| Field | Type | |
| --- | --- | --- |
| `id` | `string` | The player's UUID, `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`. |
| `name` | `string` |  |
| `x` | `f64` | Feet position, in blocks. |
| `y` | `f64` |  |
| `z` | `f64` |  |
| `dimension` | `string` | `minecraft:overworld`, `minecraft:the_nether`, ... |
| `game-mode` | `game-mode` |  |
| `health` | `f32` | Hit points as vanilla counts them: 20 is full health without effects. |

### record `item-look`

How a stack looks, beyond its item.

| Field | Type | |
| --- | --- | --- |
| `name` | `option<string>` | Its name, as an anvil's: none keeps what it has. |
| `lore` | `list<string>` | Lines under its name. |
| `model` | `option<string>` | The item model it shows: `mymod:press_tier_2`. |
| `glint` | `option<bool>` | Forces the enchanted shine on or off. |

### record `player-event`

What happened to players since the last tick, for a mod's `tick`.

| Field | Type | |
| --- | --- | --- |
| `player` | `string` |  |
| `kind` | `string` | `died`, `respawned`, `changed-dimension` (detail: the new one), `crafted` (detail: the recipe), `tossed`, `used-item`, `broke-item` (detail: the item) or `targeted` (detail: the mob's id). With no player, `block-gone` (detail: `dimension x y z`): a storage's block went, however it did. New kinds may appear: ignore others. |
| `detail` | `string` |  |

### record `item-stack`

An item stack, as server mods hand it around (inventories, block storages, item events).

| Field | Type | |
| --- | --- | --- |
| `item` | `string` | `minecraft:iron_ingot`. |
| `count` | `u32` |  |
| `data` | `list<u8>` | The mod's own bytes on the stack (up to 1024), kept in the item's `minecraft:custom_data` and saved with it: every mod sees only its own. Stacks with different bytes do not stack together. |
| `components` | `list<u8>` | Everything else the stack carries (enchantments, damage, a name, other mods' bytes), opaque: hand it back unchanged when moving the stack; empty for a plain new stack. |

| Function | Signature | What it does |
| --- | --- | --- |
| `style-stack` | `func(stack: item-stack, look: item-look) -> result<item-stack, string>` | The same stack with this look, seen by every client. |
| `set-undying` | `func(player: string, on: bool) -> result<_, string>` | While on, a blow that would kill this player leaves them on 1 health, as a totem does with no totem used and no effects. Until the world closes. |
| `sneaking` | `func(player: string) -> bool` | Whether a player is holding sneak, as of the tick's start. |
| `look` | `func(player: string) -> option<tuple<f32, f32>>` | Where the player looks: yaw and pitch in degrees, as F3 shows them. |
| `get-data` | `func(player: string, key: string) -> option<list<u8>>` | The mod's bytes on a player under `key`, as `set-data` left them: saved with the world, kept through death and leaving (NeoForge's data attachments). Each mod sees only its own. |
| `set-data` | `func(player: string, key: string, data: list<u8>) -> result<_, string>` | Keeps up to 65536 bytes on a player, up to 64 keys per mod and player. Empty `data` removes the key. |
| `poll-events` | `func() -> list<player-event>` | This tick's player events. Asking twice in a tick gives them twice. |
| `online` | `func() -> list<player>` | The players online, in the order they joined. |
| `tell` | `func(player: string, text: string) -> bool` | A system message to one player, after the mod's name; false when no player with that id is online. Up to 8 messages per call. |
| `broadcast` | `func(text: string)` | A system message to every player and the server's log, after the mod's name. Counts toward the same 8 per call. |
| `inventory` | `func(player: string) -> list<option<item-stack>>` | The player's inventory as of this tick's start, 41 slots: 0-8 the hotbar, 9-35 the main inventory, 36-39 armor (feet, legs, chest, head), 40 the off hand. Empty when no such player is online. |
| `selected-slot` | `func(player: string) -> option<u32>` | The hotbar slot the player holds (0-8); none when not online. |
| `give` | `func(player: string, stack: item-stack) -> result<_, string>` | Gives a stack as `/give` does: into the inventory, the rest dropped at the player's feet. Changes to inventories are made in order at the tick's end. Err for an unknown item or a player not online. |
| `set-slot` | `func(player: string, slot: u32, stack: option<item-stack>) -> result<_, string>` | Puts a stack in a slot (`inventory`'s numbering), none empties it. |
| `take` | `func(player: string, item: string, count: u32) -> u32` | Takes up to `count` of `item` (any bytes and components) from the inventory, as `/clear` does: how many it holds of it now, so how many go at the tick's end. |
| `by-name` | `func(name: string) -> option<player>` | The online player with this name. |
| `by-id` | `func(id: string) -> option<player>` | The online player with this id. |
| `is-online` | `func(id: string) -> bool` | Whether a player with this id is online. |
| `count` | `func() -> u32` | How many players are online. |
| `near` | `func(dimension: string, x: f64, y: f64, z: f64, radius: f64) -> list<player>` | The players in a dimension within `radius` blocks, nearest first. |
| `held-item` | `func(player: string) -> option<item-stack>` | What the player holds in the main hand, as of the tick's start. |
| `offhand-item` | `func(player: string) -> option<item-stack>` | What the player holds in the off hand, as of the tick's start. |
| `armor` | `func(player: string) -> list<option<item-stack>>` | What the player wears: feet, legs, chest, head. Empty when not online. |
| `count-item` | `func(player: string, item: string) -> u32` | How many of `item` the whole inventory holds. |
| `has-item` | `func(player: string, item: string, count: u32) -> bool` | Whether the inventory holds at least `count` of `item`. |
| `free-slots` | `func(player: string) -> u32` | Empty slots in the hotbar and main inventory. |
| `find-item` | `func(player: string, item: string) -> option<u32>` | The first slot (`inventory`'s numbering) holding `item`. |
| `set-game-mode` | `func(player: string, mode: game-mode) -> result<_, string>` | `/gamemode`. |
| `clear-inventory` | `func(player: string) -> result<_, string>` | `/clear`: empties the whole inventory. |
| `show-title` | `func(player: string, title: string, subtitle: option<string>) -> result<_, string>` | `/title`: a big line on the player's screen, and a smaller one under it. |
| `action-bar` | `func(player: string, text: string) -> result<_, string>` | `/title actionbar`: a line over the hotbar. |
| `set-title-times` | `func(player: string, fade-in: u32, stay: u32, fade-out: u32) -> result<_, string>` | `/title times`: fade in, stay and fade out, in ticks. |
| `add-experience` | `func(player: string, points: s32) -> result<_, string>` | `/experience add`: experience points; negative takes them away. |
| `add-levels` | `func(player: string, levels: s32) -> result<_, string>` | `/experience add`: whole levels; negative takes them away. |
| `set-spawn` | `func(player: string, dimension: string, x: s32, y: s32, z: s32) -> result<_, string>` | `/spawnpoint`: where the player respawns. |
| `kick` | `func(player: string, reason: string) -> result<_, string>` | `/kick`. Single player's owner can't be kicked, as in vanilla. |
| `play-sound` | `func(player: string, sound: string, volume: f32, pitch: f32) -> result<_, string>` | `/playsound`: a sound only this player hears, where they stand. |
| `stop-sounds` | `func(player: string) -> result<_, string>` | `/stopsound`: stops every sound this player hears. |
| `grant-advancement` | `func(player: string, advancement: string) -> result<_, string>` | `/advancement grant ... only`. |
| `revoke-advancement` | `func(player: string, advancement: string) -> result<_, string>` | `/advancement revoke ... only`. |

## `world`

Access: capability `world`.

Capability `world`, server mods only (manifest `api = "0.3"`, experimental): the blocks of the server's loaded chunks, and changes to them through the server's own block updates.

### record `block-change`

| Field | Type | |
| --- | --- | --- |
| `x` | `s32` |  |
| `y` | `s32` |  |
| `z` | `s32` |  |
| `state` | `string` | `minecraft:oak_log[axis=y]`; properties left out take the block's defaults, `minecraft:stone` alone is the default state. |

| Function | Signature | What it does |
| --- | --- | --- |
| `get-block` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<string>` | The block state at a position in a dimension (`minecraft:overworld`, ...), as `minecraft:oak_log[axis=y]`, as of this call: none where the server has no chunk loaded there. |
| `get-biome` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<string>` | The biome at a position (`minecraft:plains`, ...), as the server stores it (one per 4 x 4 x 4 cell): none where no chunk is loaded. |
| `set-blocks` | `func(dimension: string, blocks: list<block-change>) -> result<_, string>` | Sets blocks as `/setblock` does: the old block entity removed, neighbours and shapes updated, light, and every player told. Every state is checked now (an error queues nothing); the changes are made in order after this call returns, before that dimension's next tick; one in a chunk not loaded then is skipped. At most 4096 per call. |
| `get-data` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<list<u8>>` | The mod's own bytes on the block at a position, as `set-data` left them: none when it kept none there, where no chunk is loaded, or when the block is no longer the one they were set on (broken or replaced since; its properties may change, a furnace lit or not). Only this mod's data: every mod has its own. |
| `set-data` | `func(dimension: string, x: s32, y: s32, z: s32, data: list<u8>) -> result<_, string>` | Keeps `data` (up to 4096 bytes) on the block at a position: saved with the world, gone once that block is broken or replaced. Empty `data` removes it. Err where no chunk is loaded, on air, for too much, or past 65536 positions per mod. |
| `data-positions` | `func(dimension: string) -> list<tuple<s32, s32, s32>>` | Every position in `dimension` where the mod keeps data (some may be stale until read: `get-data` says), for a tick over its machines. |
| `schedule-tick` | `func(dimension: string, x: s32, y: s32, z: s32, delay: u32) -> result<_, string>` | Calls `block-events.block-tick` for this mod at a position after `delay` server ticks (1 to 72000), as long as the same block is there then: a machine's timer, a growing crop. Saved with the world. Err where no chunk is loaded, or past 65536 waiting per mod. |
| `redstone-power` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<u8>` | The strongest redstone signal reaching the block at a position from its neighbours (0 to 15, `getBestNeighborSignal`), as of the tick's start. Known for blocks a server mod keeps data or a storage on; none elsewhere. |
| `set-redstone-output` | `func(dimension: string, x: s32, y: s32, z: s32, signal: u8) -> result<_, string>` | The block at a position gives this weak redstone signal (0 to 15) on every side: wires, lamps and pistons beside it hear it from the next tick. Saved with the world, gone with the block; 0 stops it. Works in the level the server ticks last. |
| `set-comparator-output` | `func(dimension: string, x: s32, y: s32, z: s32, signal: u8) -> result<_, string>` | What a comparator reads from the block at a position (0 to 15), as a chest's fill level: saved and gone as `set-redstone-output`. |
| `get-saved` | `func(key: string) -> option<list<u8>>` | The mod's own bytes saved with this world under `key`, as `set-saved` left them: what a world has learned or owns, unlike `storage`, which is the same in every world. Each mod sees only its own. Worlds lend these after `init`: read them from the first tick on. |
| `set-saved` | `func(key: string, data: list<u8>) -> result<_, string>` | Keeps `data` (up to 1 MiB) under `key` (up to 64 keys per mod) in this world's save. Empty `data` removes it. |
| `mod-loaded` | `func(id: string) -> bool` | Whether a mod with this id is running: for optional links between mods (NeoForge's `ModList.isLoaded`). Known from the first tick on. |
| `mod-send` | `func(target: string, channel: string, data: list<u8>) -> result<_, string>` | Leaves a message for a running mod, which reads it with `mod-receive`. Up to 64 KiB each, and 1024 waiting per mod. |
| `mod-receive` | `func() -> list<tuple<string, string, list<u8>>>` | The messages waiting for this mod, oldest first, as (sender, channel, data). They are not saved. |
| `item-has-tag` | `func(item: string, tag: string) -> bool` | Whether an item is in an item tag: `c:ingots`, `minecraft:logs`. |
| `block-has-tag` | `func(block: string, tag: string) -> bool` | Whether a block is in a block tag: `minecraft:mineable/pickaxe`. |
| `entity-has-tag` | `func(kind: string, tag: string) -> bool` | Whether an entity type is in an entity type tag: `minecraft:undead`. |
| `set-locked-results` | `func(items: list<string>) -> result<_, string>` | Items no crafting grid makes while this list stands (per mod, the last call wins; everyone's lists add up), for mods that open recipes by progress. Vanilla's and other mods' recipes alike; at most 4096. |
| `get-blocks` | `func(dimension: string, positions: list<tuple<s32, s32, s32>>) -> result<list<option<string>>, string>` | Block states at many positions at once, in order: cheaper than asking one by one. None where no chunk is loaded. At most 4096. |
| `is-loaded` | `func(dimension: string, x: s32, z: s32) -> bool` | Whether the chunk holding this column is loaded. |
| `is-air` | `func(dimension: string, x: s32, y: s32, z: s32) -> bool` | Whether the block is air of any kind; false where no chunk is loaded. |
| `block-id` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<string>` | The block's id without its properties: `minecraft:oak_log`. |
| `block-property` | `func(dimension: string, x: s32, y: s32, z: s32, name: string) -> option<string>` | One property of the block's state: `axis` of an oak log gives `y`. |
| `set-block` | `func(dimension: string, x: s32, y: s32, z: s32, state: string) -> result<_, string>` | `set-blocks` for one block. |
| `set-block-property` | `func(dimension: string, x: s32, y: s32, z: s32, name: string, value: string) -> result<_, string>` | Changes one property of the block's state, keeping the others. |
| `break-block` | `func(dimension: string, x: s32, y: s32, z: s32, drop: bool) -> result<_, string>` | Removes a block as `/setblock ... destroy` does, with its drops and particles when `drop`, or quietly. |
| `fill` | `func(dimension: string, x1: s32, y1: s32, z1: s32, x2: s32, y2: s32, z2: s32, state: string) -> result<_, string>` | Sets every block of a box, corners included: at most 4096. |
| `find-blocks` | `func(dimension: string, x1: s32, y1: s32, z1: s32, x2: s32, y2: s32, z2: s32, block: string, limit: u32) -> result<list<tuple<s32, s32, s32>>, string>` | Positions in a box holding `block` (an id, or a full state), up to `limit` (at most 4096). A box of up to 65536 blocks. |
| `count-blocks` | `func(dimension: string, x1: s32, y1: s32, z1: s32, x2: s32, y2: s32, z2: s32, block: string) -> result<u32, string>` | How many blocks of a box hold `block`. A box of up to 65536 blocks. |
| `height-at` | `func(dimension: string, x: s32, z: s32) -> option<s32>` | The y of the highest block that isn't air in a column. |
| `game-time` | `func() -> option<u64>` | Ticks the world has run, as of the tick's start. |
| `day-time` | `func() -> option<u64>` | The time of day (0 to 24000 is one day, and it goes on counting). |
| `set-time` | `func(ticks: u32) -> result<_, string>` | `/time set`. |
| `set-weather` | `func(weather: string, seconds: u32) -> result<_, string>` | `/weather`: `clear`, `rain` or `thunder` for `seconds` (at least 1). |
| `set-gamerule` | `func(rule: string, value: string) -> result<_, string>` | `/gamerule`: `keep_inventory`, `true`. |
| `set-difficulty` | `func(difficulty: string) -> result<_, string>` | `/difficulty`: `peaceful`, `easy`, `normal` or `hard`. |
| `set-world-spawn` | `func(x: s32, y: s32, z: s32) -> result<_, string>` | `/setworldspawn`. |
| `lightning` | `func(dimension: string, x: f64, y: f64, z: f64) -> result<_, string>` | Strikes lightning at a position. |
| `explode` | `func(dimension: string, x: f64, y: f64, z: f64, power: f32) -> result<_, string>` | An explosion of `power` (a creeper's is 3, TNT's 4), at most 16, as game rules allow, asked of `game-events.explosion`. |
| `has-data` | `func(dimension: string, x: s32, y: s32, z: s32) -> bool` | Whether the mod keeps data on this block. |
| `saved-keys` | `func() -> list<string>` | The keys this mod has under `set-saved`. |
| `running-mods` | `func() -> list<string>` | The ids of the running mods. |
| `is-valid-state` | `func(state: string) -> bool` | Whether a string names a block state: check before storing one. |
| `item-exists` | `func(item: string) -> bool` | Whether the game knows this item. |
| `max-stack-size` | `func(item: string) -> option<u32>` | An item's stack size; none for an item the game doesn't know. |
| `random-u32` | `func() -> u32` | A random number, different on each call (the sandbox has no clock to seed one from). |

## `storages`

Access: capability `world`.

Capability `world`: items, energy and fluids kept on blocks, which every server mod can move in and out (a pipe of one mod feeds a machine of another), and windows with slots for them. A storage belongs to the mod that declared it, lives on its block like `world.set-data` (saved with the world, gone with the block) and works in every dimension. Vanilla containers (chests, barrels, hoppers, furnaces, shulker boxes...) answer the item calls too, in the loaded level.

### type `item-stack`

`type item-stack = item-stack`

### record `fluid-stack`

A fluid amount in millibuckets (1000 per bucket).

| Field | Type | |
| --- | --- | --- |
| `fluid` | `string` | `minecraft:water`, `minecraft:lava`, or any id a mod uses. |
| `amount` | `u32` |  |

### record `storage-shape`

What a storage holds: slots (up to 54), energy (FE, the unit NeoForge mods use) and tanks (up to 16), each with its capacity in mB.

| Field | Type | |
| --- | --- | --- |
| `slots` | `u32` |  |
| `energy-capacity` | `u64` |  |
| `tanks` | `list<u32>` |  |

### record `slot-rule`

What players may do with a slot of the mod's storage in a window.

| Field | Type | |
| --- | --- | --- |
| `take-only` | `bool` | Nothing goes in by hand: an output. |
| `items` | `list<string>` | The only items that go in; empty: any. |

| Function | Signature | What it does |
| --- | --- | --- |
| `declare` | `func(dimension: string, x: s32, y: s32, z: s32, shape: storage-shape) -> result<_, string>` | Gives the block at a position a storage of this shape, for this mod. Declared again with another shape it keeps what fits. Err where no chunk is loaded, on air, on another mod's storage or a vanilla container, or past 65536 storages per mod. |
| `remove` | `func(dimension: string, x: s32, y: s32, z: s32)` | Removes this mod's storage there, and what it held. |
| `shape` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<storage-shape>` | The storage's shape there (a vanilla container: its slots only). |
| `items` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<list<option<item-stack>>>` | The slots of the storage or vanilla container there. |
| `set-item` | `func(dimension: string, x: s32, y: s32, z: s32, slot: u32, stack: option<item-stack>) -> result<_, string>` | Sets a slot of the mod's own storage (no limits but the slot count). |
| `insert-item` | `func(dimension: string, x: s32, y: s32, z: s32, stack: item-stack, simulate: bool) -> u32` | Moves a stack in, as a hopper does: into slots holding the same item first, then empty ones, up to each item's stack size. How many went in (or would, when `simulate`). |
| `extract-item` | `func(dimension: string, x: s32, y: s32, z: s32, item: option<string>, count: u32, simulate: bool) -> option<item-stack>` | Takes up to `count` from the first slot holding `item` (none: any item), as a hopper does. What came out (or would). |
| `energy` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<tuple<u64, u64>>` | Stored and capacity, in FE. |
| `insert-energy` | `func(dimension: string, x: s32, y: s32, z: s32, amount: u64, simulate: bool) -> u64` | How much went in (up to the capacity), or would. |
| `extract-energy` | `func(dimension: string, x: s32, y: s32, z: s32, amount: u64, simulate: bool) -> u64` | How much came out, or would. |
| `set-energy` | `func(dimension: string, x: s32, y: s32, z: s32, amount: u64) -> result<_, string>` | Sets the mod's own storage's energy (clamped to its capacity). |
| `fluids` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<list<option<fluid-stack>>>` | Each tank's fluid, none when empty. |
| `insert-fluid` | `func(dimension: string, x: s32, y: s32, z: s32, fluid: fluid-stack, simulate: bool) -> u32` | Fills tanks holding the same fluid first, then empty ones: how many mB went in, or would. |
| `extract-fluid` | `func(dimension: string, x: s32, y: s32, z: s32, fluid: option<string>, amount: u32, simulate: bool) -> option<fluid-stack>` | Drains up to `amount` mB from the first tank holding `fluid` (none: any). |
| `set-fluid` | `func(dimension: string, x: s32, y: s32, z: s32, tank: u32, fluid: option<fluid-stack>) -> result<_, string>` | Sets a tank of the mod's own storage (clamped to its capacity). |
| `open-menu` | `func(player: string, dimension: string, x: s32, y: s32, z: s32, title: string) -> result<_, string>` | Opens a window with the storage's slots and the player's inventory, as a chest's: the server runs every click, shift-click and drag, so the mod only reads its slots. The window has rows of 9 (a storage of 9, 18, ... 54 slots) or is a hopper's (5 slots); `title` is plain text. It closes when the player leaves or the storage goes. Err for another shape, or a player not online. |
| `open-menu-as` | `func(player: string, dimension: string, x: s32, y: s32, z: s32, title: string, kind: string) -> result<_, string>` | `open-menu` with the window chosen by name: `minecraft:generic_9x1` to `generic_9x6`, `generic_3x3`, `shulker_box` (27 slots), `hopper` (5), `furnace`, `blast_furnace`, `smoker` (3). Err when the storage's slots don't fit it, or for another name. |
| `set-menu-data` | `func(dimension: string, x: s32, y: s32, z: s32, data: list<s16>) -> result<_, string>` | A storage of 3 slots opens as a furnace's window (input, fuel, output); these are its bars: flame left, flame total, arrow progress, arrow total. Sent to whoever has the window open when they change. Err for another mod's storage or not 4 values. |
| `set-slot-rule` | `func(dimension: string, x: s32, y: s32, z: s32, slot: u32, rule: option<slot-rule>) -> result<_, string>` | Sets a slot's rule; none lifts it. What a window's player puts in against it goes back to their inventory. Mods' own calls ignore rules. Err for another mod's storage or a slot it doesn't have. |
| `has-storage` | `func(dimension: string, x: s32, y: s32, z: s32) -> bool` | Whether a mod's storage is on this block (vanilla containers: no). |
| `is-empty` | `func(dimension: string, x: s32, y: s32, z: s32) -> bool` | Whether the storage or vanilla container there holds nothing at all. |
| `is-full` | `func(dimension: string, x: s32, y: s32, z: s32) -> bool` | Whether every slot there holds a full stack. |
| `free-slots` | `func(dimension: string, x: s32, y: s32, z: s32) -> u32` | Empty slots there. |
| `count-item` | `func(dimension: string, x: s32, y: s32, z: s32, item: string) -> u32` | How many of `item` the slots there hold. |
| `clear` | `func(dimension: string, x: s32, y: s32, z: s32) -> result<_, string>` | Empties the mod's own storage: items, energy and fluids. |
| `positions` | `func(dimension: string) -> list<tuple<s32, s32, s32>>` | Every position in `dimension` of this mod's storages. |
| `free-energy` | `func(dimension: string, x: s32, y: s32, z: s32) -> u64` | How much more energy fits there, in FE. |
| `fluid-amount` | `func(dimension: string, x: s32, y: s32, z: s32, fluid: string) -> u32` | How many mB of `fluid` the tanks there hold. |
| `transfer-item` | `func(dimension: string, from: tuple<s32, s32, s32>, to: tuple<s32, s32, s32>, item: option<string>, count: u32) -> u32` | Moves up to `count` of `item` (none: any) from one block to another as a hopper would, only as much as fits: how many moved. |
| `transfer-energy` | `func(dimension: string, from: tuple<s32, s32, s32>, to: tuple<s32, s32, s32>, amount: u64) -> u64` | Moves up to `amount` FE, only as much as fits: how much moved. |
| `transfer-fluid` | `func(dimension: string, from: tuple<s32, s32, s32>, to: tuple<s32, s32, s32>, fluid: option<string>, amount: u32) -> u32` | Moves up to `amount` mB of `fluid` (none: any): how many moved. |

## `entities`

Access: capability `entities`.

Capability `entities` (server mods): the mobs of the loaded level, and the server's own commands for entities, effects, particles and sounds. Changes are made in order at the end of the call's tick, as the server console would make them.

### type `item-stack`

`type item-stack = item-stack`

### record `entity`

| Field | Type | |
| --- | --- | --- |
| `id` | `string` | The UUID, `xxxxxxxx-xxxx-...`; a player's is `players`'s id. |
| `kind` | `string` | `minecraft:zombie`. |
| `x` | `f64` |  |
| `y` | `f64` |  |
| `z` | `f64` |  |
| `health` | `f32` |  |

| Function | Signature | What it does |
| --- | --- | --- |
| `near` | `func(dimension: string, x: f64, y: f64, z: f64, radius: f64) -> list<entity>` | The living mobs within `radius` (up to 128) blocks of a position, as of the tick's start, nearest first, at most 256. Mobs of the level the server ticks last (single player: the owner's) only. |
| `spawn` | `func(dimension: string, kind: string, x: f64, y: f64, z: f64, nbt: string) -> result<_, string>` | `/summon`: `kind` at a position, with `nbt` (SNBT, empty for none). |
| `hurt` | `func(id: string, amount: f32) -> result<_, string>` | `/damage`: hurts an entity or player by its id. |
| `get-data` | `func(id: string, key: string) -> option<list<u8>>` | The mod's bytes on an entity (or player) by id: saved with the world, the data attachments of NeoForge. Each mod sees only its own. |
| `set-data` | `func(id: string, key: string, data: list<u8>) -> result<_, string>` | Up to 65536 bytes, 64 keys per mod and entity; empty removes the key. |
| `set-ai` | `func(id: string, on: bool) -> result<_, string>` | Turns a mob's own AI off (`NoAI`) or back on, so a mod can drive it. |
| `set-name` | `func(id: string, name: option<string>) -> result<_, string>` | The name over a mob's head; none removes it. |
| `set-velocity` | `func(id: string, x: f64, y: f64, z: f64) -> result<_, string>` | Sets a mob's motion, in blocks per tick. A mob with its AI off doesn't move: `teleport` drives it. |
| `set-rotation` | `func(id: string, yaw: f32, pitch: f32) -> result<_, string>` | `/rotate`: which way an entity faces, in degrees: how a display mob (`"display": true` in its type) turns its moving parts. |
| `move-to` | `func(id: string, x: f64, y: f64, z: f64, speed: f64) -> result<_, string>` | Sends a mob walking to a position at `speed` times its own (1.0 is its walk), as a goal would. Its goals may send it elsewhere later. |
| `set-target` | `func(id: string, target: option<string>) -> result<_, string>` | The entity a mob attacks: a mob's or player's id; none: no one. |
| `set-equipment` | `func(id: string, slot: string, stack: item-stack) -> result<_, string>` | `/item replace entity`: what a mob or player wears or holds. `slot`: `weapon.mainhand`, `weapon.offhand`, `armor.head`, `armor.chest`... A stack's `data` isn't carried. |
| `drop-item` | `func(dimension: string, x: s32, y: s32, z: s32, stack: item-stack) -> result<_, string>` | Drops a stack on the ground as an item at a block, at the end of the tick (of that dimension's next turn, if it isn't in place). |
| `set-attribute` | `func(id: string, attribute: string, value: f64) -> result<_, string>` | `/attribute`: sets the base value of an attribute (`minecraft:max_health`) of an entity or player. |
| `kill` | `func(id: string) -> result<_, string>` | `/kill`. |
| `teleport` | `func(id: string, dimension: string, x: f64, y: f64, z: f64) -> result<_, string>` | `/tp`: to a position in a dimension. |
| `effect` | `func(id: string, effect: string, seconds: u32, amplifier: u8) -> result<_, string>` | `/effect give`: `effect` (`minecraft:speed`) for `seconds` at `amplifier` (0 is level I). |
| `particle` | `func(dimension: string, particle: string, x: f64, y: f64, z: f64, dx: f32, dy: f32, dz: f32, speed: f32, count: u32) -> result<_, string>` | `/particle`: `count` of `particle` (`minecraft:flame`) spread over `dx dy dz`, at `speed`, seen by every player in range. |
| `sound` | `func(dimension: string, sound: string, x: f64, y: f64, z: f64, volume: f32, pitch: f32) -> result<_, string>` | `/playsound`: `sound` (`minecraft:block.anvil.use`) at a position for every player in range. |
| `get` | `func(dimension: string, id: string) -> option<entity>` | A living mob of a dimension by its id, as of the tick's start. |
| `exists` | `func(dimension: string, id: string) -> bool` | Whether that mob is alive there. |
| `of-kind` | `func(dimension: string, kind: string, x: f64, y: f64, z: f64, radius: f64) -> list<entity>` | `near`, only mobs of `kind`. |
| `nearest` | `func(dimension: string, x: f64, y: f64, z: f64, radius: f64, kind: option<string>) -> option<entity>` | The nearest mob within `radius`, of `kind` if given. |
| `count-near` | `func(dimension: string, x: f64, y: f64, z: f64, radius: f64) -> u32` | How many mobs are within `radius` (up to 128). |
| `clear-effects` | `func(id: string) -> result<_, string>` | `/effect clear`: every effect of an entity or player. |
| `remove-effect` | `func(id: string, effect: string) -> result<_, string>` | `/effect clear`: one effect of an entity or player. |
| `heal` | `func(id: string, amount: f32) -> result<_, string>` | Heals a mob, up to its maximum health. |
| `set-health` | `func(id: string, health: f32) -> result<_, string>` | Sets a mob's health, from above 0 up to its maximum. |
| `swing` | `func(id: string) -> result<_, string>` | `/swing`: an entity or player swings its main hand. |

## `worldgen`

Access: capability `worldgen`.

Capability `worldgen`, server mods only (experimental): world generation modifiers. Today one surface block per biome, applied to the chunks the game decorates while the rule is set.

### record `ore`

A vein of ore, in the chunks the game decorates while it is set.

| Field | Type | |
| --- | --- | --- |
| `biome` | `string` | `minecraft:plains`, or `*` for every biome. |
| `block` | `string` | What the vein is made of. |
| `replaces` | `list<string>` | What it may replace (`minecraft:stone`, ...). |
| `min-y` | `s32` |  |
| `max-y` | `s32` |  |
| `size` | `u32` | Blocks in a vein, 1 to 64. |
| `per-chunk` | `u32` | Veins tried per chunk, 1 to 64. |

| Function | Signature | What it does |
| --- | --- | --- |
| `set-surface-rule` | `func(biome: string, surface: string) -> result<_, string>` | The block the game puts at the top of `biome`'s terrain, e.g. `minecraft:stone`. The last rule for a biome wins. |
| `add-ore` | `func(ore: ore) -> result<_, string>` | Adds veins of ore to new chunks. A mod may add up to 64. |

## `server-events`

Access: exported by the mod: the game calls it.

What the server calls in a server mod (manifest `api = "0.3"`, experimental). Only the hooks `init` asks for are ever called. The player in each event is as the server saw them when it happened.

### type `player`

`type player = player`

### record `block-pos`

| Field | Type | |
| --- | --- | --- |
| `x` | `s32` |  |
| `y` | `s32` |  |
| `z` | `s32` |  |

### variant `argument-kind`

What an argument of a command takes. The server sends each command and its arguments to players' games, whose chat box completes and colours them as vanilla's; the server parses them before the mod's `command` hook gets their values.

| Case | Payload | |
| --- | --- | --- |
| `word` |  | One word: letters, digits and `_ - . +`. |
| `text` |  | The rest of the line, spaces included: only last. |
| `integer` | `tuple<option<s32>, option<s32>>` | A whole number, within the bounds given. |
| `number` |  | A number with or without decimals. |
| `boolean` |  | `true` or `false`. |
| `player` |  | One online player: a name, or `@s` or `@p` (whoever typed it, as vanilla picks for a player). |
| `block-pos` |  | A block position: three whole numbers, each one also `~` or `~n`, relative to whoever typed it. |

### record `argument`

| Field | Type | |
| --- | --- | --- |
| `name` | `string` | Shown in the chat box's hint (`<height>`) and the key of its value. |
| `kind` | `argument-kind` |  |
| `optional` | `bool` | Optional arguments come after every required one. |

### record `server-command`

| Field | Type | |
| --- | --- | --- |
| `name` | `string` | Without the slash. A name vanilla already uses is refused (vanilla's command runs). |
| `arguments` | `list<argument>` | In order; none: the command takes nothing after its name. Free text is one optional `text` argument. |

### variant `argument-value`

An argument's value, parsed as its kind says.

| Case | Payload | |
| --- | --- | --- |
| `word` | `string` |  |
| `text` | `string` |  |
| `integer` | `s32` |  |
| `number` | `f64` |  |
| `boolean` | `bool` |  |
| `player` | `player` |  |
| `block-pos` | `block-pos` |  |

### record `registration`

| Field | Type | |
| --- | --- | --- |
| `commands` | `list<server-command>` | Commands players may type. |
| `tick` | `bool` | Call `tick` every server tick (20 per second). |
| `joins` | `bool` | Call `player-joined` and `player-left`. |
| `chat` | `bool` | Call `chat` for each chat message a player sends. |
| `blocks` | `bool` | Call `block-broken` and `block-placed`. |

| Function | Signature | What it does |
| --- | --- | --- |
| `init` | `func() -> result<registration, string>` | Once, after loading. An error keeps the mod off and is logged. |
| `tick` | `func(tick: u64)` | Each server tick, with the server's tick counter. |
| `player-joined` | `func(player: player)` | A player joined the server; they are in `players.online` already. |
| `player-left` | `func(player: player)` | A player left the server; they are no longer in `players.online`. |
| `chat` | `func(player: player, text: string)` | A chat message a player sent, as plain text. |
| `command` | `func(player: player, name: string, args: string, values: list<tuple<string, argument-value>>) -> result<option<string>, string>` | `/name args` typed by `player`: ok(some(text)) is their feedback, err(text) their error in red. `values` holds each argument given, by name, in order (an optional one left out isn't there); a line that doesn't parse never reaches the mod: the player gets vanilla's error. |
| `block-broken` | `func(player: player, pos: block-pos, state: string)` | A player broke the block at `pos` in their dimension: `state` is what was there, as `minecraft:oak_log[axis=y]`. |
| `block-placed` | `func(player: player, pos: block-pos, state: string)` | A player placed `state` at `pos` in their dimension. |

## `block-events`

Access: exported by the mod: the game calls it.

Asked before the server acts on a block: what a server mod answers decides it (manifest `api = "0.3"`, the mod's registration has `blocks`). A server mod built before this interface has no such export and is not asked. The block hasn't changed yet when the mod is asked.

### type `player`

`type player = player`

### type `item-stack`

`type item-stack = item-stack`

| Function | Signature | What it does |
| --- | --- | --- |
| `break-block` | `func(player: player, x: s32, y: s32, z: s32, state: string) -> bool` | `player` starts breaking the block at `x y z` in their dimension; `state` is what is there, as `minecraft:oak_log[axis=y]`. True refuses it: the block stays, and the player's game puts it back, as under spawn protection. Every mod asked may refuse. |
| `use-block` | `func(player: player, x: s32, y: s32, z: s32, state: string, item: string) -> bool` | `player` right-clicks the block at `x y z` (`state`) holding `item` in the hand they use (`minecraft:air`: an empty hand). True: the mod handled it, and vanilla neither uses the block (doors, chests, buttons...) nor uses or places the held item there. The first mod (in load order) that answers true takes it. |
| `place-block` | `func(player: player, x: s32, y: s32, z: s32, item: string) -> bool` | `player` is about to place `item` (a block item) at `x y z`, the block it would go in. True refuses it: nothing is placed and the item stays. Every mod asked may refuse. |
| `use-item` | `func(player: player, stack: item-stack, off-hand: bool) -> bool` | `player` right-clicks the air holding `stack` (`off-hand`: in the off hand). True: the mod handled it and vanilla does not use the item. |
| `block-tick` | `func(dimension: string, x: s32, y: s32, z: s32, state: string)` | The tick `world.schedule-tick` asked for, the block still `state`. |
| `neighbor-changed` | `func(dimension: string, x: s32, y: s32, z: s32, state: string, from-x: s32, from-y: s32, from-z: s32)` | A block next to one this mod keeps data or a storage on changed, found at the tick's start: `state` is the mod's block, `from-*` the neighbour. |

## `game-events`

Access: exported by the mod: the game calls it.

What happens to entities and the world, for server mods that list the event in their manifest (`events = ["entity-hurt", ...]`). Mods are asked in order of their manifest `priority` (higher first, then load order); for an event that can be cancelled, the first mod that answers true cancels it and the mods after it are not asked. Every event but `explosion` is found at the end of the server tick it happened in, in the level ticked last (single player: the owner's), so a cancel undoes it there: a spawned mob is removed, a hurt mob gets its health back. A server mod built before this interface has no such export and is not asked.

### type `player`

`type player = player`

### type `item-stack`

`type item-stack = item-stack`

### type `entity`

`type entity = entity`

| Function | Signature | What it does |
| --- | --- | --- |
| `entity-spawned` | `func(dimension: string, entity: entity) -> bool` | A mob appeared (natural spawn, egg, breeding, `/summon`, another mod). True removes it. |
| `entity-hurt` | `func(dimension: string, entity: entity, amount: f32) -> bool` | A mob lost `amount` health this tick and lives. True gives it back. |
| `entity-died` | `func(dimension: string, entity: entity, killer: option<string>)` | A mob died; `killer` is the player who last hurt it, if one did. |
| `player-hurt` | `func(player: player, amount: f32)` | A player lost `amount` health this tick. |
| `item-picked-up` | `func(player: player, stack: item-stack)` | A player picked up `stack` from the ground. |
| `explosion` | `func(dimension: string, x: f64, y: f64, z: f64, power: f32) -> bool` | An explosion of `power` (TNT: 4) is about to go off: true stops it, with no damage and no blocks broken. Asked as it happens. |

## `before-events`

Access: exported by the mod: the game calls it.

Asked before something happens, as it happens, of the server mods whose manifest lists the event with `before-` in front (`events = ["before-entity-hurt", ...]`), in order of `priority`. A hook that refuses stops the asking: the mods after it are not asked. A hook that changes a value hands the changed value to the next mod. What a mod does from here (`entities`, `world`...) waits for the tick's end, as always.

### type `player`

`type player = player`

### type `item-stack`

`type item-stack = item-stack`

### type `entity`

`type entity = entity`

| Function | Signature | What it does |
| --- | --- | --- |
| `entity-spawn` | `func(dimension: string, entity: entity, reason: string) -> bool` | A mob is about to join the level in place: `reason` is `natural`, `chunk` (made with new terrain), `spawner`, `mob` (born of another: breeding, a slime splitting, a zombie's reinforcements) or `summoned` (a spawn egg, `/summon`, a raid, a patrol, an iron golem, a warden, another mod). True refuses it: the mob never appears. |
| `entity-hurt` | `func(dimension: string, entity: entity, damage-type: string, attacker: option<string>, amount: f32) -> f32` | A mob is about to lose `amount` health to `damage-type` (`minecraft:arrow`), past invulnerability, before armor; `attacker` is the mob's or player's id when one dealt it. The answer is what it loses instead: `amount` to leave it, 0 or less refuses the hit. Asked inside the mob's tick for fire, drowning, falls and the like: there `world` reads answer nothing, the hook sees only its arguments. Not asked yet for `/damage` and a conduit's attack. |
| `player-hurt` | `func(player: player, damage-type: string, attacker: option<string>, amount: f32) -> f32` | The same for a player, before armor and enchantments: the attacker is the mob behind a mob's hit, none for a player's (PvP). |
| `player-death` | `func(player: player, damage-type: string) -> bool` | A player's health is about to reach 0 (after a totem had its say): true keeps them alive on 1 health. |
| `explosion-blocks` | `func(dimension: string, x: f64, y: f64, z: f64, blocks: list<tuple<s32, s32, s32>>) -> list<tuple<s32, s32, s32>>` | An explosion at x y z will break `blocks`, after `game-events.explosion` let it go off: the answer is the ones that stay. Up to 4096 are asked of; the rest break. |
| `item-pickup` | `func(player: player, stack: item-stack) -> bool` | `player` is about to pick up `stack` from the ground: true leaves it there. Asked again each tick they stand by it. |
| `item-toss` | `func(player: player, stack: item-stack) -> bool` | `player` is about to drop `stack` with the drop key (Q, or ctrl+Q for all of it): true keeps it in their inventory. |
| `mob-drop` | `func(dimension: string, entity: entity, killer: option<string>, drops: list<item-stack>) -> option<list<item-stack>>` | What a dead mob is about to drop, by `killer` when a player killed it: `none` leaves it, a list is what drops instead (up to 64 stacks). |
| `xp-drop` | `func(dimension: string, x: f64, y: f64, z: f64, source: string, amount: u32) -> u32` | Experience about to drop at x y z: `source` is `mob` (a dead mob's, asked after `mob-drop`). The answer is what drops. |
| `container-open` | `func(player: player, x: s32, y: s32, z: s32, block: string) -> bool` | `player` is about to open the window of `block` at x y z (a chest, barrel, ender chest, shulker box, hopper, dispenser, dropper, furnace, brewing stand or crafter): true keeps it shut. |

## `entity-events`

Access: exported by the mod: the game calls it.

Asked as they happen, of every server mod built since this interface: the first that answers true takes it, and vanilla does nothing with it.

### type `player`

`type player = player`

### type `entity`

`type entity = entity`

| Function | Signature | What it does |
| --- | --- | --- |
| `use-entity` | `func(player: player, entity: entity, item: string) -> bool` | A right click on an entity, holding `item` (`minecraft:air`: empty hand). A mob, a player (`minecraft:player`, id: the player's) or an object (boat, minecart, armor stand, item frame, painting; id: its network id, which `entities` calls don't take). |
| `attack-entity` | `func(player: player, entity: entity, item: string) -> bool` | An attack on an entity, holding `item`. As `use-entity`. |
| `entity-dying` | `func(dimension: string, entity: entity, killer: option<string>) -> bool` | A mob's health reached 0; `killer` is the player who last hurt it. True keeps it alive on 1 health. Only for mods listing `entity-dying` in the manifest's `events`. |
| `projectile-hit` | `func(dimension: string, projectile: entity, target: option<string>, x: f64, y: f64, z: f64) -> bool` | A mob type with `projectile` hit a block or, with `target`'s id, a living thing, at x y z. True keeps it flying; otherwise it is gone. |

## `logic-events`

Access: exported by the mod: the game calls it.

Code behind what a mod declares in data; each is asked only of the mod whose id is the declaration's namespace.

### type `entity`

`type entity = entity`

### record `chunk-fill`

What a chunk holds: `blocks` index `palette` (`ns:block[a=b]`), 256 a layer, x then z in a layer, layers from `min-y` up. 0 in `palette` may be `minecraft:air`.

| Field | Type | |
| --- | --- | --- |
| `palette` | `list<string>` |  |
| `min-y` | `s32` |  |
| `blocks` | `list<u16>` |  |
| `biome` | `option<string>` |  |

| Function | Signature | What it does |
| --- | --- | --- |
| `generate-chunk` | `func(dimension: string, x: s32, z: s32) -> option<chunk-fill>` | A chunk of a dimension declared with `"generator": {"type": "code"}`. |
| `goal-can-use` | `func(goal: string, mob: entity) -> bool` | Whether a mob may start `goal` (a name in its type's `goals`), asked every few ticks for goals above the one it runs. While one runs the mob's vanilla goals wait; drive it with `entities`. |
| `goal-tick` | `func(goal: string, mob: entity) -> bool` | A running goal's tick: false ends it. |
| `goal-stop` | `func(goal: string, mob: entity)` | A goal ended or was replaced by one above it. |
| `entity-tick` | `func(dimension: string, entity: entity)` | Each tick of a mob whose type says `"ticks": true`. |
| `effect-tick` | `func(effect: string, target: string, amplifier: u8)` | Each tick of a mob or player under this mod's effect, level `amplifier`. |

## `client-net`

Access: capability `net`.

Capability `net` for a client mod: its own messages to the server's mods, over the connection (single player's own server included). A channel is `[a-z0-9_./-]`, 1 to 64 characters; mods agree on it (use the mod's id in it). Each message is up to 32 KiB, 64 per call.

| Function | Signature | What it does |
| --- | --- | --- |
| `send` | `func(channel: string, data: list<u8>) -> result<_, string>` | Sends `data` on `channel` to the server's mods (`server-net-events`). Err for a bad channel or too much; on a server that isn't Horizon's the message is dropped there. |

## `client-net-events`

Access: exported by the mod: the game calls it.

What a client mod with capability `net` hears from the server's mods.

| Function | Signature | What it does |
| --- | --- | --- |
| `received` | `func(channel: string, data: list<u8>)` | A message the server's mods sent on `channel`. |

## `server-net`

Access: capability `net`.

Capability `net` for a server mod: messages to players' client mods.

| Function | Signature | What it does |
| --- | --- | --- |
| `send` | `func(player: string, channel: string, data: list<u8>) -> result<_, string>` | Sends `data` on `channel` to one player's client mods. Err for a bad channel, too much, or a player not online. |
| `broadcast` | `func(channel: string, data: list<u8>) -> result<_, string>` | The same to every player. |

## `server-net-events`

Access: exported by the mod: the game calls it.

What a server mod with capability `net` hears from players' client mods.

### type `player`

`type player = player`

| Function | Signature | What it does |
| --- | --- | --- |
| `received` | `func(player: player, channel: string, data: list<u8>)` | A message `player`'s client mods sent on `channel`. |
