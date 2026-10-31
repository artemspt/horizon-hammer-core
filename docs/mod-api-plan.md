# Mod API plan: the next functions

The list for extending the mod API, most useful first. Tick a box in the commit that lands it. How to work: `crates/horizon-mods/AGENTS.md`.

Names are proposals: rename when it reads better, and leave out one that turns out useless, with a word why. Nothing here was checked against
the game's code one by one, so some will turn out bigger than their kind says, and a few impossible: say so here rather than faking them.

**The goal is sections 1 to 14, about 460 functions.** Sections 15 to 17 are in reserve, for after them or when one is needed.

**Kinds**, by where the work is:

- **A**: the runtime only, built from what the API already reads (`players.count-item`). Fast.
- **B**: a wrapper over a vanilla command (`players.set-game-mode`). Fast, if Horizon has the command: check with `/help` or `crates/horizon-client/src/game/commands.rs`.
- **C**: a new hook or action in the game. Medium.
- **D**: client or engine work. Slow.

The first 75 (API level 18) went fast because nearly all were A and B. Most of this list is C, so expect it to go slower.

Hook names here live in the interface that fits; a hook is an export, so put it in an events interface and its host world, not among imports. A new interface needs a capability: see `of_import` in `crates/horizon-mods/src/manifest.rs`.

## 1. Events asked before they happen (30, kind C)

Today spawn and hurt are found at the tick's end, so a cancel undoes the result. Mods need to change damage and refuse things as they happen. Top priority: most big mods lean on this.

Landed hooks live in their own interface, `before-events` (world `before-events-host`), not in `game-events`: a mod built
before them still binds its `game-events`. A manifest lists them as `before-<name>`. The game asks most of them where the
thing happens (`game/mod_before.rs`); damage is asked inside the mob's own tick, where `Game::mod_gated` lends the mods
to `horizon_client::mod_gate` for the phase.

- [x] `game-events.before-entity-spawn`: may refuse a mob spawning, natural or not (`before-events.entity-spawn`, with a reason: natural, chunk, spawner, mob, summoned)
- [x] `game-events.before-entity-hurt`: may refuse or change the damage, with its type and attacker (`before-events.entity-hurt`, asked inside the mob's tick through `horizon_client::mod_gate`; not yet for `/damage` and a conduit)
- [x] `game-events.before-player-hurt`: the same for players (`before-events.player-hurt`, in `PlayerMotion::hurt_caused_by`, so fire, drowning and falls too)
- [x] `game-events.before-entity-death`: may refuse a death, for mobs (already there as `entity-events.entity-dying` (level 17): no second hook)
- [x] `game-events.before-player-death`: may refuse a death, for players (`before-events.player-death`, in the totem's place)
- [x] `game-events.before-explosion-blocks`: which blocks an explosion breaks: may remove some (`before-events.explosion-blocks`: the answer is the blocks that stay)
- [x] `game-events.before-item-pickup`: may refuse a player picking up an item (`before-events.item-pickup`, guests)
- [x] `game-events.before-item-toss`: may refuse a player dropping an item (`before-events.item-toss`, the drop key; a refusal sends the inventory again)
- [ ] `game-events.before-teleport`: may refuse or move a teleport (waits: `/tp`, pearls and chorus fruit each move the player their own way)
- [ ] `game-events.before-dimension-change`: may refuse a change of dimension (waits: portals for guests run in `server_travel.rs`, not read yet)
- [ ] `game-events.before-sleep`: may refuse a bed (waits: a guest's bed goes through the hub, not read yet)
- [ ] `game-events.before-effect-added`: may refuse an effect (waits: effects go on in eight places in the game and in mob code; needs the gate around beacons and drinking)
- [ ] `game-events.before-block-drop`: may change what a broken block drops (waits: drops of a broken block are spawned in the hub's dig code)
- [x] `game-events.before-mob-drop`: may change what a dead mob drops (`before-events.mob-drop`)
- [x] `game-events.before-xp-drop`: may change the experience dropped (`before-events.xp-drop`, a dead mob's so far (`source` = `mob`))
- [ ] `game-events.before-crop-grow`: may refuse a growth step
- [ ] `game-events.before-sapling-grow`: may refuse or replace a tree
- [ ] `game-events.before-fluid-place`: may refuse a fluid source being placed
- [ ] `game-events.before-fire-spread`: may refuse fire spreading
- [ ] `game-events.before-farmland-trample`: may refuse trampling
- [ ] `game-events.before-piston-move`: may refuse a piston moving blocks
- [ ] `game-events.before-portal-create`: may refuse a nether portal forming
- [ ] `game-events.before-mob-target`: may refuse or change a mob's target
- [ ] `game-events.before-mob-griefing`: may refuse a mob changing blocks
- [x] `game-events.before-container-open`: may refuse a window opening (`before-events.container-open`: chests, barrels, hoppers, furnaces, brewing stands, crafters; not yet crafting tables, anvils and the other stations)
- [ ] `game-events.before-craft`: may refuse or change a crafted result
- [ ] `game-events.before-smelt`: may refuse a furnace's step
- [ ] `game-events.before-anvil`: may set an anvil's result and cost
- [ ] `game-events.before-enchant`: may change an enchanting table's result
- [ ] `game-events.before-chat`: may refuse or rewrite a chat line (waits: the hub broadcasts a guest's chat before the game hears it (`HostEvent::PlayerChat`))

## 2. Events that only tell (35, kind C)

Hooks missing today; each comes with its entry in `manifest::EVENTS`, so a mod not asking pays nothing.

- [ ] `game-events.block-exploded`: a block went in an explosion
- [ ] `game-events.block-burned`: a block burned away
- [ ] `game-events.block-grown`: a crop or sapling grew
- [ ] `game-events.block-moved`: a piston moved a block
- [ ] `game-events.leaves-decayed`: leaves decayed
- [ ] `game-events.fluid-flowed`: a fluid reached a new block
- [ ] `game-events.chunk-loaded`: a chunk loaded, with its dimension
- [ ] `game-events.chunk-unloaded`: a chunk unloaded
- [ ] `game-events.chunk-generated`: a chunk was made for the first time
- [ ] `game-events.level-loaded`: a dimension opened
- [ ] `game-events.level-unloaded`: a dimension closed
- [ ] `game-events.level-saved`: the world saved: flush what the mod holds
- [ ] `game-events.weather-changed`: rain or thunder began or ended
- [ ] `game-events.day-changed`: a new day began
- [ ] `game-events.player-advancement`: a player earned an advancement
- [ ] `game-events.player-level-up`: a player gained a level
- [ ] `game-events.player-slept`: a player got into or out of bed
- [ ] `game-events.player-swapped-hands`: a player swapped hands
- [ ] `game-events.player-selected-slot`: a player changed the held slot
- [ ] `game-events.player-jumped`: a player jumped
- [ ] `game-events.player-landed`: a player landed, with the fall distance
- [ ] `game-events.player-sprint-changed`: a player started or stopped sprinting
- [ ] `game-events.player-opened-container`: a player opened a window
- [ ] `game-events.player-closed-container`: a player closed a window
- [ ] `game-events.player-fished`: a player caught something
- [ ] `game-events.player-traded`: a player traded with a villager
- [ ] `game-events.player-ate`: a player finished eating
- [ ] `game-events.player-bred`: a player bred animals
- [ ] `game-events.entity-mounted`: an entity got on or off a ride
- [ ] `game-events.entity-tamed`: a mob was tamed
- [ ] `game-events.entity-effect-ended`: an effect ran out
- [ ] `game-events.item-expired`: an item on the ground despawned
- [ ] `game-events.projectile-fired`: a projectile was shot, with its shooter
- [ ] `game-events.command-run`: a player ran any command
- [ ] `game-events.mod-reloaded`: this mod was reloaded with `/hmod reload`

## 3. Items: reading and building stacks (40, kind A/C)

Every mod with items needs this. `style-stack` covers only the look; components are opaque today.

- [ ] `items.exists-in-tag`: every item in a tag
- [ ] `items.all`: every item id
- [ ] `items.name`: an item's translated name
- [ ] `items.max-damage`: an item's durability
- [ ] `items.is-damageable`: whether it wears out
- [ ] `items.is-food`: whether it can be eaten
- [ ] `items.food-values`: nutrition and saturation
- [ ] `items.is-block-item`: whether it places a block
- [ ] `items.block-of`: the block it places
- [ ] `items.fuel-ticks`: how long it burns in a furnace
- [ ] `items.rarity`: an item's rarity
- [ ] `items.components`: a stack's components as JSON
- [ ] `items.get-component`: one component of a stack
- [ ] `items.set-component`: a stack with one component set
- [ ] `items.remove-component`: a stack with one component removed
- [ ] `items.get-damage`: a stack's damage
- [ ] `items.set-damage`: a stack with this damage
- [ ] `items.repair`: a stack repaired by an amount
- [ ] `items.get-custom-name`: a stack's name
- [ ] `items.get-lore`: a stack's lore
- [ ] `items.same-item`: whether two stacks are the same item
- [ ] `items.can-stack`: whether two stacks stack together
- [ ] `items.split`: a stack split in two
- [ ] `items.grow`: a stack with its count changed, within limits
- [ ] `items.enchantments`: a stack's enchantments and levels
- [ ] `items.enchantment-level`: one enchantment's level on a stack
- [ ] `items.set-enchantment`: a stack with an enchantment set
- [ ] `items.remove-enchantment`: a stack without an enchantment
- [ ] `items.has-glint`: whether a stack shines
- [ ] `items.attribute-modifiers`: a stack's attribute modifiers
- [ ] `items.add-attribute-modifier`: a stack with a modifier added
- [ ] `items.tool-speed`: how fast a tool breaks a block
- [ ] `items.is-correct-tool`: whether a tool gets a block's drops
- [ ] `items.set-cooldown`: an item's cooldown for a player
- [ ] `items.get-cooldown`: ticks left on it
- [ ] `items.dyed-color`: a stack's dye colour
- [ ] `items.set-dyed-color`: a stack with a dye colour
- [ ] `items.potion-contents`: a potion's effects
- [ ] `items.set-potion-contents`: a potion with these effects
- [ ] `items.container-contents`: what a shulker box or bundle holds

## 4. Item behaviour in code (20, kind C)

Custom tools, guns, staffs and tooltips: the core of tech and magic mods.

- [ ] `item-events.use-tick`: each tick while an item is held in use
- [ ] `item-events.use-finished`: use ran its whole time
- [ ] `item-events.use-stopped`: use let go early, with time held
- [ ] `item-events.use-on-entity`: an item used on an entity
- [ ] `item-events.use-on-block`: an item used on a block, before the block is asked
- [ ] `item-events.left-click-air`: a left click on nothing
- [ ] `item-events.inventory-tick`: each tick an item is in an inventory, for chosen items
- [ ] `item-events.held-tick`: each tick an item is in hand, for chosen items
- [ ] `item-events.mine-block`: a tool broke a block
- [ ] `item-events.hit-entity`: a weapon hit an entity
- [ ] `item-events.tooltip`: lines a mod adds under a stack's tooltip
- [ ] `item-events.crafted`: an item was made
- [ ] `item-events.durability-used`: durability is about to drop: may change it
- [ ] `item-events.bar`: the durability bar's width and colour, for a charge bar
- [ ] `item-events.dropped`: an item left a player's hand
- [ ] `item-events.entity-tick`: each tick for a thrown item entity, for chosen items
- [ ] `players.start-using`: makes a player begin using the held item
- [ ] `players.stop-using`: ends it
- [ ] `players.is-using`: whether a player is using an item, and which
- [ ] `players.use-time`: ticks of use so far

## 5. Loot, recipes and drops (31, kind A/C)

`loot.rs` and `recipes_resolver.rs` exist but no mod reaches them. Needed by TerraFirmaCraft and GregTech analogues.

- [ ] `loot.roll`: rolls a loot table, with luck and position
- [ ] `loot.table-exists`: whether a table exists
- [ ] `loot.block-drops`: what a block would drop with a tool
- [ ] `loot.entity-drops`: what a mob would drop
- [ ] `loot.give`: rolls a table into a player's inventory
- [ ] `loot.drop`: rolls a table on the ground
- [ ] `loot.fill-container`: rolls a table into a container
- [ ] `loot.add-modifier`: adds items to a table's results
- [ ] `loot.remove-from-table`: removes an item from a table's results
- [ ] `loot.replace-in-table`: replaces one item with another in a table's results
- [ ] `loot.set-chest-loot`: gives a placed chest a table and seed
- [ ] `recipes-server.all`: every recipe id
- [ ] `recipes-server.by-type`: every recipe of a type
- [ ] `recipes-server.by-result`: every recipe making an item
- [ ] `recipes-server.by-ingredient`: every recipe taking an item
- [ ] `recipes-server.get`: one recipe as JSON
- [ ] `recipes-server.add`: adds a recipe while the world runs
- [ ] `recipes-server.remove`: removes a recipe
- [ ] `recipes-server.replace-ingredient`: swaps an ingredient everywhere
- [ ] `recipes-server.replace-result`: swaps a result everywhere
- [ ] `recipes-server.match-crafting`: what a 3 x 3 grid of stacks makes
- [ ] `recipes-server.match-smelting`: what a stack smelts into
- [ ] `recipes-server.match-stonecutting`: what a stack cuts into
- [ ] `recipes-server.smelting-time`: how long a stack smelts
- [ ] `recipes-server.smelting-xp`: the experience it gives
- [ ] `recipes-server.register-type`: a recipe type of the mod's own, read from `data/`
- [ ] `recipes-server.of-own-type`: every recipe of that type
- [ ] `recipes-server.show-in-book`: shows the mod's recipes in the recipe book
- [ ] `players.unlock-recipe`: `/recipe give`
- [ ] `players.lock-recipe`: `/recipe take`
- [ ] `players.knows-recipe`: whether the player's book has one

## 6. World generation from a mod (40, kind C)

What a Twilight Forest or TerraFirmaCraft analogue needs first. Upstream's `for_world_with` already takes worldgen overrides from data packs: reuse it. See `docs/mod-core-plan.md` section 12.

- [ ] `worldgen.add-biome`: a biome from `data/` joins a dimension's source
- [ ] `worldgen.remove-biome`: a biome stops generating
- [ ] `worldgen.replace-biome`: one biome in place of another
- [ ] `worldgen.biome-list`: every biome id
- [ ] `worldgen.biome-info`: temperature, downfall, colours
- [ ] `worldgen.set-biome-colors`: grass, foliage, water and sky colours of a biome
- [ ] `worldgen.biome-has-tag`: whether a biome is in a tag
- [ ] `worldgen.add-feature`: a placed feature in a biome and step
- [ ] `worldgen.remove-feature`: takes a feature out of a biome
- [ ] `worldgen.place-feature`: places a configured feature now
- [ ] `worldgen.add-structure`: a structure from `data/` in a set of biomes
- [ ] `worldgen.remove-structure`: stops a structure generating
- [ ] `worldgen.place-structure`: places a structure now
- [ ] `worldgen.place-template`: places an `.nbt` template now, with rotation and mirror
- [ ] `worldgen.save-template`: saves a box as a template
- [ ] `worldgen.template-size`: a template's size
- [ ] `worldgen.find-structure`: the nearest structure of a kind
- [ ] `worldgen.in-structure`: which structure holds a position
- [ ] `worldgen.find-biome`: the nearest biome of a kind
- [ ] `worldgen.add-spawn`: a mob joins a biome's spawns
- [ ] `worldgen.remove-spawn`: a mob leaves a biome's spawns
- [ ] `worldgen.set-spawn-cost`: spawn density of a mob
- [ ] `worldgen.remove-ore`: removes an ore the mod added
- [ ] `worldgen.add-disk`: a disk of clay or sand
- [ ] `worldgen.add-tree`: a tree in a biome
- [ ] `worldgen.add-flower`: a flower in a biome
- [ ] `worldgen.add-lake`: a lake in a biome
- [ ] `worldgen.add-geode`: a geode in a biome
- [ ] `worldgen.add-carver`: a cave carver in a biome
- [ ] `worldgen.remove-carver`: takes a carver out
- [ ] `worldgen.set-noise-settings`: a dimension's noise settings from `data/`
- [ ] `worldgen.set-default-block`: a dimension's stone
- [ ] `worldgen.set-default-fluid`: a dimension's sea fluid
- [ ] `worldgen.set-sea-level`: a dimension's sea level
- [ ] `worldgen.decorate-chunk`: a hook: the mod changes each new chunk, in any dimension
- [ ] `worldgen.generate-overworld`: `logic-events.generate-chunk` for the overworld too
- [ ] `worldgen.column-height`: the terrain's height from noise, before generation
- [ ] `worldgen.seed`: the world's seed
- [ ] `worldgen.set-fillbiome`: `/fillbiome` in a box
- [ ] `worldgen.locate-poi`: the nearest point of interest of a kind

## 7. Dimensions and portals (20, kind A/C)

Mod dimensions work; getting into them and how they look does not.

- [ ] `dimensions.list`: every dimension, the mods' included
- [ ] `dimensions.exists`: whether one exists
- [ ] `dimensions.loaded`: whether one is open now
- [ ] `dimensions.base`: the vanilla type under a mod's dimension
- [ ] `dimensions.min-y`: its lowest block
- [ ] `dimensions.max-y`: its highest block
- [ ] `dimensions.time`: its own time of day
- [ ] `dimensions.set-time`: sets it
- [ ] `dimensions.players-in`: the players there
- [ ] `dimensions.keep-loaded`: keeps it ticking with no one there
- [ ] `dimensions.register-portal`: a frame block and light item make a portal to a dimension
- [ ] `dimensions.teleport-rule`: where arrivals land: same x z, scaled, or fixed
- [ ] `dimensions.set-fog-color`: its fog colour
- [ ] `dimensions.set-sky-color`: its sky colour
- [ ] `dimensions.set-skybox`: a texture for its sky
- [ ] `dimensions.set-ambient-light`: its darkness
- [ ] `dimensions.set-has-sky`: sky light on or off
- [ ] `dimensions.set-has-weather`: weather on or off
- [ ] `dimensions.set-bed-works`: beds work or explode
- [ ] `dimensions.set-respawn`: respawn anchors work or not

## 8. Mobs and entities: state (60, kind A/B/C)

Reads and writes on mobs mods keep writing by hand.

- [ ] `entities.kind`: an entity's type by id
- [ ] `entities.position`: where it is
- [ ] `entities.rotation`: which way it faces
- [ ] `entities.velocity`: its motion
- [ ] `entities.health`: its health
- [ ] `entities.max-health`: its maximum
- [ ] `entities.dimension`: where it is, across levels
- [ ] `entities.is-alive`: alive or not
- [ ] `entities.is-on-ground`: on the ground or not
- [ ] `entities.is-in-water`: in water or not
- [ ] `entities.is-in-lava`: in lava or not
- [ ] `entities.is-burning`: on fire or not
- [ ] `entities.is-baby`: a baby or not
- [ ] `entities.set-baby`: makes it a baby or an adult
- [ ] `entities.is-tamed`: tamed or not
- [ ] `entities.owner`: who tamed it
- [ ] `entities.set-owner`: tames it for a player
- [ ] `entities.custom-name`: the name over its head
- [ ] `entities.effects`: its effects, with level and time
- [ ] `entities.has-effect`: whether it has one
- [ ] `entities.attribute`: an attribute's current value
- [ ] `entities.base-attribute`: its base value
- [ ] `entities.add-attribute-modifier`: a modifier by id
- [ ] `entities.remove-attribute-modifier`: removes it
- [ ] `entities.equipment`: what it wears and holds
- [ ] `entities.target`: what it attacks
- [ ] `entities.passengers`: who rides it
- [ ] `entities.vehicle`: what it rides
- [ ] `entities.ride`: makes it ride another
- [ ] `entities.dismount`: gets it off
- [ ] `entities.remove`: removes it with no death or drops
- [ ] `entities.set-fire`: sets it alight for some ticks
- [ ] `entities.extinguish`: puts it out
- [ ] `entities.set-glowing`: an outline through walls
- [ ] `entities.set-invisible`: invisible or not
- [ ] `entities.set-invulnerable`: can't be hurt
- [ ] `entities.set-silent`: makes no sound
- [ ] `entities.set-no-gravity`: floats
- [ ] `entities.set-persistent`: never despawns
- [ ] `entities.set-pickup-delay`: how long a dropped item waits
- [ ] `entities.add-tag`: `/tag add`
- [ ] `entities.remove-tag`: `/tag remove`
- [ ] `entities.tags`: its tags
- [ ] `entities.has-tag`: whether it has one
- [ ] `entities.get-nbt`: its NBT as SNBT
- [ ] `entities.merge-nbt`: `/data merge entity`
- [ ] `entities.look-at`: turns it to face a position
- [ ] `entities.knockback`: pushes it back from a position
- [ ] `entities.add-velocity`: adds to its motion
- [ ] `entities.line-of-sight`: whether one entity sees another
- [ ] `entities.distance`: between two entities
- [ ] `entities.in-box`: every mob in a box
- [ ] `entities.raycast`: the first entity along a ray
- [ ] `entities.all`: every mob of a level
- [ ] `entities.items-near`: dropped items near a position, with their stacks
- [ ] `entities.pickup-item`: a mob or player takes a dropped item
- [ ] `entities.xp-orb`: drops experience at a position
- [ ] `entities.spawn-with-reason`: spawn, saying why, with a returned id
- [ ] `entities.spawn-group`: several mobs with one call
- [ ] `entities.can-spawn-at`: whether a mob fits at a position

## 9. Mob AI and bodies (30, kind C)

Code goals exist; goals from vanilla and pathing don't. See section 12 of the core plan.

- [ ] `mob-ai.add-goal`: a vanilla goal by name and priority: `melee_attack`, `stroll`...
- [ ] `mob-ai.remove-goal`: takes one out
- [ ] `mob-ai.clear-goals`: takes every goal out, the template's too
- [ ] `mob-ai.goals`: a mob's goals
- [ ] `mob-ai.add-target-goal`: a vanilla targeting goal
- [ ] `mob-ai.clear-target-goals`: takes them out
- [ ] `mob-ai.set-goal-flags`: a code goal holds move, look or jump only
- [ ] `mob-ai.navigate`: a path to a position, with its result
- [ ] `mob-ai.stop-navigation`: stops walking
- [ ] `mob-ai.is-navigating`: whether it walks
- [ ] `mob-ai.path-exists`: whether a path exists, before walking
- [ ] `mob-ai.look-at`: turns the head for a time
- [ ] `mob-ai.jump`: makes it jump
- [ ] `mob-ai.attack`: makes it attack now
- [ ] `mob-ai.shoot`: makes a ranged mob shoot at a target
- [ ] `mob-ai.set-aggressive`: the aggressive pose
- [ ] `mob-ai.set-speed`: its speed for a time
- [ ] `mob-ai.flee-from`: runs from an entity kind
- [ ] `mob-ai.set-home`: stays near a position
- [ ] `mob-ai.memory-get`: a brain memory by name
- [ ] `mob-ai.memory-set`: sets one
- [ ] `mob-ai.play-animation`: a named animation from the mod's model
- [ ] `mob-ai.stop-animation`: stops it
- [ ] `mob-ai.set-model`: changes a mob's model
- [ ] `mob-ai.set-texture`: changes a mob's texture
- [ ] `mob-ai.set-scale`: its size
- [ ] `mob-ai.set-hitbox`: its width and height
- [ ] `mob-ai.add-part`: a linked part with its own hitbox, for bosses
- [ ] `mob-ai.remove-part`: takes it off
- [ ] `mob-ai.boss-bar`: a mob's boss bar, tied to its health

## 10. Blocks: state and behaviour (42, kind A/C)

Reads every machine and worldgen mod needs, and hooks on what happens to a block.

- [ ] `blocks.all`: every block id
- [ ] `blocks.properties`: a block's properties and their values
- [ ] `blocks.default-state`: its default state
- [ ] `blocks.hardness`: how hard it is
- [ ] `blocks.resistance`: its blast resistance
- [ ] `blocks.light-emission`: the light it gives
- [ ] `blocks.light-at`: light level at a position
- [ ] `blocks.sky-light-at`: sky light at a position
- [ ] `blocks.block-light-at`: block light at a position
- [ ] `blocks.can-see-sky`: whether a position sees the sky
- [ ] `blocks.is-solid`: solid or not
- [ ] `blocks.is-replaceable`: can be placed over or not
- [ ] `blocks.is-opaque`: blocks light or not
- [ ] `blocks.has-collision`: can be walked through or not
- [ ] `blocks.collision-boxes`: its shape's boxes
- [ ] `blocks.requires-tool`: needs the right tool for drops or not
- [ ] `blocks.map-color`: its map colour
- [ ] `blocks.sound-type`: its sound set
- [ ] `blocks.piston-reaction`: how a piston treats it
- [ ] `blocks.fluid-at`: the fluid at a position, with level
- [ ] `blocks.is-waterlogged`: waterlogged or not
- [ ] `blocks.has-block-entity`: has a block entity or not
- [ ] `blocks.get-nbt`: a block entity's NBT
- [ ] `blocks.merge-nbt`: `/data merge block`
- [ ] `blocks.tick-now`: asks a block's random tick now
- [ ] `blocks.neighbor-update`: tells neighbours of a change
- [ ] `blocks.drop-contents`: drops a container's contents
- [ ] `blocks.drops`: what it would drop with a tool
- [ ] `blocks.can-place`: whether a state survives at a position
- [ ] `blocks.place-as-player`: places as a player would, with sounds and rules
- [ ] `blocks.break-as-player`: breaks as a player would, with tool wear
- [ ] `blocks.destroy-progress`: shows cracks at a position
- [ ] `blocks.fall`: makes a block fall as sand does
- [ ] `blocks.set-falling`: makes a mod's block fall by itself
- [ ] `blocks.random-tick`: a hook: random ticks for chosen blocks
- [ ] `blocks.entity-inside`: a hook: an entity stands in or on chosen blocks
- [ ] `blocks.stepped-on`: a hook: an entity steps on chosen blocks
- [ ] `blocks.fell-on`: a hook: an entity lands on chosen blocks
- [ ] `blocks.projectile-hit`: a hook: a projectile hits chosen blocks
- [ ] `blocks.piston-pushed`: a hook: a piston moves chosen blocks
- [ ] `blocks.exploded`: a hook: an explosion takes chosen blocks
- [ ] `blocks.block-attack`: a hook: a player left-clicks chosen blocks

## 11. Chunks and batch world access (20, kind B/C)

Performance first: see `docs/mod-core-known-issues.md`. Worth measuring before and after.

- [ ] `chunks.read-section`: a 16 x 16 x 16 section at once, as state ids
- [ ] `chunks.state-id`: a state's number, for fast reads
- [ ] `chunks.state-of-id`: the state a number means
- [ ] `chunks.write-section`: writes a section at once
- [ ] `chunks.biomes`: a chunk's biomes
- [ ] `chunks.heightmap`: a chunk's top blocks
- [ ] `chunks.is-generated`: whether a chunk exists in the save
- [ ] `chunks.force-load`: `/forceload add`
- [ ] `chunks.force-unload`: `/forceload remove`
- [ ] `chunks.forced`: which chunks are kept loaded
- [ ] `chunks.loaded`: every loaded chunk of a dimension
- [ ] `chunks.get-data`: the mod's bytes on a chunk, saved with the world (radiation, pollution)
- [ ] `chunks.set-data`: sets them
- [ ] `chunks.data-chunks`: every chunk where the mod keeps data
- [ ] `chunks.subscribe-changes`: lists changed blocks in chosen boxes, without polling
- [ ] `chunks.poll-changes`: what changed there since the last tick
- [ ] `chunks.schedule-many`: many block timers in one call
- [ ] `chunks.get-data-many`: many blocks' data in one call
- [ ] `chunks.set-data-many`: writes many blocks' data in one call
- [ ] `chunks.relight`: recomputes light in a box

## 12. Machines, networks and storages (32, kind A/C)

Lets the game do the heavy work natively. Biggest win for GregTech and AE2 analogues.

- [ ] `networks.create`: a network of a kind: energy, items or fluids
- [ ] `networks.remove`: removes one
- [ ] `networks.add-node`: a position joins it, with input, output and buffer limits
- [ ] `networks.remove-node`: it leaves
- [ ] `networks.nodes`: what a network holds
- [ ] `networks.stats`: moved per tick, stored, capacity
- [ ] `networks.of-block`: which network a position is in
- [ ] `networks.set-priority`: which node is served first
- [ ] `networks.set-tier`: a voltage or pipe tier: what it passes per tick
- [ ] `networks.on-overload`: a hook: a node took too much
- [ ] `storages.set-side-config`: which side takes and gives what
- [ ] `storages.side-config`: reads it
- [ ] `storages.auto-output`: pushes to a neighbour by itself each tick
- [ ] `storages.auto-input`: pulls from a neighbour by itself each tick
- [ ] `storages.set-filter`: which items a storage takes
- [ ] `storages.set-capacity`: changes a slot's size limit
- [ ] `storages.transfer-all`: moves everything that fits
- [ ] `storages.neighbors`: which of six sides hold storages or containers
- [ ] `storages.total-energy`: stored energy over many positions
- [ ] `storages.sort`: sorts a storage's slots
- [ ] `storages.compact`: merges its stacks
- [ ] `storages.drop-all`: drops what it holds, without removing it
- [ ] `storages.changed`: a hook: a storage's slots changed
- [ ] `storages.save-to-stack`: a stack keeping a storage's contents
- [ ] `storages.load-from-stack`: a storage taking them back
- [ ] `menus.set-progress`: a bar in a window of the mod's layout
- [ ] `menus.set-text`: a line of text in it
- [ ] `menus.set-button`: a button in it
- [ ] `menus.button-pressed`: a hook: it was pressed
- [ ] `menus.close`: closes a player's window
- [ ] `menus.open-for`: who has a storage's window open
- [ ] `menus.set-tooltip`: a tooltip over a region of it

## 13. Players (43, kind A/B/C)

What a survival or RPG mod reaches for.

- [ ] `players.food`: hunger and saturation
- [ ] `players.set-food`: sets them
- [ ] `players.add-exhaustion`: makes them hungrier
- [ ] `players.air`: air left
- [ ] `players.set-air`: sets it
- [ ] `players.max-health`: their maximum
- [ ] `players.heal`: heals a player
- [ ] `players.set-health`: sets their health
- [ ] `players.xp`: level, progress and total
- [ ] `players.set-xp`: sets them
- [ ] `players.position`: where they stand, fresh
- [ ] `players.velocity`: their motion
- [ ] `players.set-velocity`: sets it
- [ ] `players.is-sprinting`: sprinting or not
- [ ] `players.is-flying`: flying or not
- [ ] `players.set-flying`: starts or stops flight
- [ ] `players.set-may-fly`: allows flight in survival
- [ ] `players.is-on-ground`: on the ground or not
- [ ] `players.is-sleeping`: in bed or not
- [ ] `players.is-swimming`: swimming or not
- [ ] `players.is-gliding`: gliding or not
- [ ] `players.is-op`: an operator or not
- [ ] `players.ping`: their latency
- [ ] `players.raycast-block`: the block they look at
- [ ] `players.raycast-entity`: the entity they look at
- [ ] `players.open-container`: opens a chest or crafting table window
- [ ] `players.close-window`: closes what they have open
- [ ] `players.ender-chest`: their ender chest's slots
- [ ] `players.set-ender-chest-slot`: sets one
- [ ] `players.spawn-point`: their respawn point
- [ ] `players.statistic`: a statistic by name
- [ ] `players.has-advancement`: whether they have an advancement
- [ ] `players.set-selected-slot`: changes the held slot
- [ ] `players.drop-held`: makes them drop what they hold
- [ ] `players.swing-hand`: swings their arm
- [ ] `players.teleport-look`: teleports with a yaw and pitch
- [ ] `players.particle-to`: a particle only they see
- [ ] `players.tell-json`: a chat component to one player
- [ ] `players.broadcast-json`: a chat component to everyone
- [ ] `players.set-tab-header`: the player list's header and footer
- [ ] `players.set-display-name`: the name others see
- [ ] `players.set-permission`: a mod's own permission on a player
- [ ] `players.has-permission`: reads it

## 14. Effects, damage and attributes of a mod's own (20, kind C)

Section 12 item 5 of the core plan.

- [ ] `damage.deal`: hurts with a damage type and an attacker
- [ ] `damage.types`: every damage type
- [ ] `damage.register-type`: a damage type from `data/`
- [ ] `damage.is-type`: whether damage is of a kind or tag
- [ ] `damage.armor-reduced`: what armor leaves of an amount
- [ ] `damage.last-source`: what last hurt an entity
- [ ] `effects.all`: every effect
- [ ] `effects.info`: an effect's colour and category
- [ ] `effects.give`: an effect with hidden particles and ambient flags
- [ ] `effects.time-left`: ticks left on an entity
- [ ] `effects.level`: its level
- [ ] `attributes.all`: every attribute
- [ ] `attributes.register`: an attribute of the mod's own
- [ ] `attributes.default`: a base value on a mob type
- [ ] `enchantments.all`: every enchantment
- [ ] `enchantments.register`: an enchantment from `data/`
- [ ] `enchantments.max-level`: its highest level
- [ ] `enchantments.applies-to`: whether it fits an item
- [ ] `enchantments.level-on`: the best level an entity wears
- [ ] `enchantments.hook`: a hook: a chosen enchantment hit, mined or ticked

# Reserve

## 15. Server, scoreboards and teams (38, kind B/C)

Mini-games and server mods. Check each command exists in Horizon; if not, it needs game work.

- [ ] `scoreboard.add-objective`: a new objective
- [ ] `scoreboard.remove-objective`: removes one
- [ ] `scoreboard.objectives`: every objective
- [ ] `scoreboard.get`: a score
- [ ] `scoreboard.set`: sets a score
- [ ] `scoreboard.add`: adds to a score
- [ ] `scoreboard.reset`: clears a score
- [ ] `scoreboard.display`: shows an objective in a slot
- [ ] `teams.create`: a new team
- [ ] `teams.remove`: removes one
- [ ] `teams.join`: an entity or player joins
- [ ] `teams.leave`: leaves
- [ ] `teams.of`: which team an entity is on
- [ ] `teams.set-color`: a team's colour
- [ ] `teams.set-friendly-fire`: friendly fire on or off
- [ ] `bossbar.create`: a boss bar
- [ ] `bossbar.remove`: removes one
- [ ] `bossbar.set-value`: its fill
- [ ] `bossbar.set-color`: its colour
- [ ] `bossbar.set-style`: its notches
- [ ] `bossbar.set-name`: its title
- [ ] `bossbar.show-to`: who sees it
- [ ] `border.size`: the world border's size
- [ ] `border.set`: sets it, with a time to get there
- [ ] `border.center`: sets its centre
- [ ] `server.tps`: ticks per second
- [ ] `server.tick-time`: how long a tick took
- [ ] `server.max-players`: the player limit
- [ ] `server.motd`: the server's message
- [ ] `server.is-dedicated`: dedicated or not
- [ ] `server.stop`: stops the server, with a capability of its own
- [ ] `server.save-all`: saves the world
- [ ] `server.run-command-as`: a command as a player, with its reply
- [ ] `server.run-command-reply`: a console command, with its reply: lets mods see failures
- [ ] `commands.add-argument-type`: a command argument of the mod's own, with suggestions
- [ ] `commands.suggest`: suggestions for an argument, by code
- [ ] `commands.set-permission`: the level a mod's command needs
- [ ] `commands.alias`: another name for a command

## 16. Client: rendering and input (50, kind C/D)

Needs the client half (`side = both`). Upstream moved rendering to `horizon-client-render`. Custom shaders stay out of the sandbox for now.

- [ ] `render.load-obj`: an OBJ model from `assets/`, baked to polygons
- [ ] `render.set-model-speed`: a part's spin without resending the model
- [ ] `render.set-part-transform`: moves or turns one part
- [ ] `render.show-item`: an item floating in the world, not a mob
- [ ] `render.remove-item`: takes it away
- [ ] `render.spawn-particle`: a particle from the mod's `assets/`
- [ ] `render.particle-burst`: many in one call
- [ ] `render.draw-line`: a line in the world for a frame or a time
- [ ] `render.draw-box`: a box outline in the world
- [ ] `render.draw-text-3d`: text in the world
- [ ] `render.set-fog`: fog colour and distance for a time
- [ ] `render.set-sky-tint`: sky colour for a time
- [ ] `render.flash`: the screen flashes a colour
- [ ] `render.overlay`: a texture over the screen, as a pumpkin's
- [ ] `render.hide-hud-part`: hides hearts, food, hotbar or crosshair
- [ ] `render.replace-hud-part`: draws in their place
- [ ] `render.set-fov`: changes the field of view
- [ ] `render.camera-offset`: moves the camera for a time
- [ ] `render.camera-roll`: tilts it
- [ ] `render.armor-model`: a model for armor of the mod's own
- [ ] `render.item-model-3d`: an item drawn by a mod's model
- [ ] `render.block-color`: a tint for chosen blocks by code
- [ ] `render.item-color`: a tint for chosen items, for material layers
- [ ] `render.grass-color`: overrides grass colour, for seasons
- [ ] `render.foliage-color`: overrides foliage colour
- [ ] `render.play-music`: music from the mod's `assets/`
- [ ] `render.play-sound-ui`: a sound with no position
- [ ] `render.screen-size`: the window's size
- [ ] `render.is-paused`: the game paused or not
- [ ] `render.current-screen`: what screen is open
- [ ] `render.font-height`: a line's height
- [ ] `render.draw-item`: an item in the mod's screens and HUD
- [ ] `render.draw-entity`: a mob in the mod's screens
- [ ] `render.draw-texture-part`: part of a texture, stretched or tiled
- [ ] `render.scissor`: clips what a screen draws
- [ ] `input.mouse-position`: the cursor in GUI pixels
- [ ] `input.mouse-button`: whether a button is down
- [ ] `input.scroll-delta`: the wheel this frame
- [ ] `input.text-field`: a text box in a mod's screen
- [ ] `input.grab-mouse`: locks or frees the cursor
- [ ] `input.key-name`: the name of a mod's key's binding
- [ ] `input.clipboard-get`: the clipboard, with the player's consent
- [ ] `input.clipboard-set`: sets it
- [ ] `ui.add-slider`: a slider widget
- [ ] `ui.add-checkbox`: a checkbox widget
- [ ] `ui.add-tabs`: tabs in a mod's screen
- [ ] `ui.add-image`: an image widget
- [ ] `ui.toast`: a toast in the corner
- [ ] `ui.confirm`: a yes or no dialog
- [ ] `ui.tooltip-at`: a tooltip at the cursor

## 17. Assets, config and tools for authors (24, kind A/C)

Things that make a mod comfortable to write and keep.

- [ ] `config.get-string`: a setting from the manifest's `[config]`
- [ ] `config.get-int`: a number
- [ ] `config.get-bool`: a flag
- [ ] `config.set`: changes one and saves it
- [ ] `config.synced`: a setting the server sends to clients
- [ ] `storage.list`: every key the mod has stored
- [ ] `storage.read-json`: a resource from the mod's `data/` as JSON
- [ ] `storage.list-resources`: the mod's resources under a path
- [ ] `text.translate`: text in the player's language
- [ ] `text.has-translation`: whether a key has one
- [ ] `text.language`: the player's language
- [ ] `tags.items-in`: every item in a tag
- [ ] `tags.blocks-in`: every block in a tag
- [ ] `tags.entities-in`: every mob type in a tag
- [ ] `tags.of-item`: every tag holding an item
- [ ] `tags.of-block`: every tag holding a block
- [ ] `timers.after`: runs a named callback after some ticks, with no block
- [ ] `timers.every`: runs one repeatedly
- [ ] `timers.cancel`: cancels one
- [ ] `log.warn`: a warning in the log
- [ ] `log.error`: an error in the log
- [ ] `log.debug`: a line only with a dev mod
- [ ] `profiler.span`: times a piece of the mod for `/hmod profile`
- [ ] `profiler.report`: what the mod's calls cost
