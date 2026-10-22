# Inventory and paired player persistence (Slice 9)

Implemented from accepted Slice-8 main `ee16009`. This slice changes no block IDs,
texture art, frozen world generators or world-format v1–v4 bytes. Inventory and
its UI are original RustCraft code/art, MIT OR Apache-2.0.

## Model and controls

`voxel_core::inventory` owns **nine hotbar + 27 main slots** and a temporary held
cursor stack, each stack capped at 64. Five explicit item IDs map to the existing
Grass/Dirt/Stone/Wood/Leaves blocks. Private stack fields/validated constructors
prevent invalid counts; inventory operations use no Bevy or filesystem types.

Starter: Grass/Dirt/Stone 64 each, Wood/Leaves 32 each in slots 1–5, all other slots
empty. It is granted once per world's player initialization, not per restart.
Mining returns the corresponding item (Grass remains Grass); a full inventory
rejects mining atomically. There are no world drops, crafting, armor, offhand,
mouse dragging or automatic pickup entities yet.

- 1–9 selects the hotbar; wheel up selects previous, down next, with wrapping.
  Pixel-wheel events accumulate at 40 pixels/notch; focus loss discards remainder.
- E opens/closes the 3×9 inventory plus hotbar. Gameplay/look/jump/build/reset pause
  behind the modal. Cursor becomes free. Closing restores the previous capture
  state; Escape closes inventory first, a second Escape releases gameplay capture.
- Left click picks up/places/merges or swaps unlike stacks. Right click takes the
  rounded-up half, or places one into an empty/matching nonfull slot. Unlike
  stacks are not swapped by right click. Shift-left transfers to the other
  section, merging before empties and retaining overflow. Shift is ignored while
  a stack is held. Clicking outside never discards items.
- Closing returns the held stack only if it all fits; otherwise the modal stays
  open with NO ROOM. Application close persists a held stack; on restart the
  inventory opens automatically so it can be placed/returned safely.
- Every populated slot shows its actual count, including one. Empty selected
  slots cannot place. Full-inventory mining shows NO ROOM above the hotbar.

`edit_inventory` stages the inventory result, calls validated world mutation,
then commits inventory by an infallible move. Errors/no-ops leave items unchanged;
body-overlap/pending/limits checks still apply. Mesh workers never grant/consume
items. A repeated input press is one operation, not one per held frame/remesh.

## Client and original UI

`client::inventory_ui` adapts input before camera/fixed physics. Open/close frames
suppress gameplay clicks; focus loss and save-close pause input. Hit testing uses
logical window pixels and the same integer scale/layout as rendering. Windows
have a 320×240 minimum; UI scale is 1–4. Hotbar remains visible when released.

The original navy/earth/copper pixel frame, three-by-five bitmap numerals/letters
and layout are editable Rust geometry/glyph tables, not copied UI textures. Flat
16×16 item icons sample the existing original terrain atlas (grass side, dirt,
stone, upright bark, opaque leaves). UI owns **four retained mesh assets and two
materials**, no additional images. The cursor has its own two meshes/root so
pointer motion changes only transforms, not batch buffers. Other buffers rebuild
only on model/content, modal/layout or feedback changes. All are camera children,
independent of floating world origin. Lighting/terrain batching stay unchanged.

## Save and recovery protocol

`player.rcp` stores all slots, selected index and held stack. Immutable
`player-world.rcid` pins a specific world identity; v4 uses its nonce/seed/gen3,
older modes get a unique sidecar nonce plus mode/seed/generator. A root-file
fingerprint catches mismatched legacy/world generations. Exact bytes/limits are
specified in SAVE_FORMAT. No automatic player fallback or re-grant is allowed
when initialized data disappears, is corrupt, foreign or unknown.

One existing background save worker captures a coherent world snapshot and
inventory clone. F5, 30-second autosave, pressure saves and normal close include
inventory-only changes. Saved world/inventory revisions are tracked separately;
an older successful job cannot clear newer changes. UI-open state/view/player
position/velocity are not persisted. Successful held-stack load opens the UI.

Under the existing exclusive world lock:

1. Validate identity; encode a bounded checksummed `session.pending` containing
   the intended world root/full legacy data, up to eight dirty regional overlays
   and the complete intended player file. Sync/atomically commit that redo intent.
2. Validate the existing player before advancing world files. Save changed world
   data using the existing temp/backup/rename/sync protocol. Inventory-only saves
   preserve root bytes, including legacy v1. Idempotent retries preserve backups.
3. Atomically back up/replace/sync player state, then remove/sync the journal.
   A successful status requires the full sequence. Errors retain dirty RAM data
   and cancel close. World-only saving is refused after player initialization.
4. Before gameplay, replay a complete known valid journal idempotently, then load
   world/player data. This also handles an interrupted first world save without
   invoking another world/starter factory. Bad/foreign journals stop loading.

This is **save-level redo**, not a per-click journal or simultaneous filesystem
transaction. Forced termination can lose changes newer than the captured save;
a prepared older snapshot can be completed on restart. All regions/player of
that snapshot are restored together before gameplay, not partially exposed.
Disk usage/transient encoding are bounded (32-MiB journal, eight dirty regions),
but dense snapshots can use tens of MiB of temporary memory and extra disk writes.

## Recovery limits

Keep **complete closed-directory backups**, including every region, manifest,
player file, identity and any pending journal. Per-file `.bak` files are previous
generations, **not a consistent whole-world rollback**. Never restore an old
player alone against current regions or delete a pending journal to dismiss an
error. Preserve the entire damaged directory first, then recover a complete
consistent copy. Corrupt/unknown player data is never overwritten during replay.

Initial creation syncs the identity marker before preparing its first journal.
A failure in that narrow initialization gap can leave a marker without player or
journal; startup deliberately fails closed. For a genuinely fresh disposable
world use another fresh directory; preserve existing worlds and recover a complete
backup. The application never guesses a new starter over that marker.

The lock covers cooperating current clients on local filesystems; external file
edits, independently restored regions and older clients ignoring player sidecars
can break consistency (v4 root alone cannot fingerprint every historical region).
Do not use an older client on the same live inventory-enabled directory; use an
isolated copy. There is no cryptographic authenticity, remote filesystem or
absolute hardware/power-loss guarantee. Tests/measurements: VALIDATION.
