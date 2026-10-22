# Slice 9 design and implementation checkpoints

Work begins from accepted Slice 8 main `ee16009`. No block IDs, texture art,
generator versions or world v1–v4 byte contracts change.

- Engine-independent model: nine hotbar + 27 main slots, max 64, five item IDs
  explicitly mapped to the existing blocks. A separately held cursor stack is
  persisted too. Left pick/place/merge/swap; right ceil-half/one; shift transfers
  to the other section, merging before empties. Shift is ignored while holding.
- Starter inventory: Grass/Dirt/Stone 64 each, Wood/Leaves 32 each, in slots 1–5;
  remaining slots empty. Mining returns the corresponding block item (Grass
  remains Grass). A full inventory rejects mining atomically; no world drops.
- Gameplay changes stage the item result, perform validated world mutation, then
  commit the staged inventory by an infallible move. Failed/no-op world actions
  never consume/collect; mesh publication is not a gameplay callback.
- Save-level durability uses `player.rcp`, immutable `player-world.rcid`, and a
  bounded checksummed `session.pending` redo journal under the existing OS lock.
  Prepare/sync journal → save world/dirty regions → player backup/atomic replace
  → remove/sync journal. Known complete journals replay before gameplay. Unknown,
  corrupt, missing initialized or foreign data fails closed, never grants another
  starter. Legacy inventory-only changes preserve original world bytes.
- Journal snapshots include world metadata/full legacy data, at most eight dirty
  regional overlays and all inventory/selection/cursor state. Limits: 32 MiB redo,
  176-byte player file, 56-byte identity marker. Transient serialization allocations
  are bounded but can be larger than ordinary region-only saves.
- This provides redo of a captured save, not a per-click journal or a simultaneous
  filesystem transaction across all files. Unsaved edits can be lost on forced
  termination; external changes/old clients ignoring sidecars are unsupported.
  Preserve the complete closed directory in backups. Never independently restore
  an old player backup against current regions or delete a pending journal.
- Original camera-relative pixel UI will share the existing atlas image; retained
  meshes/materials update only for changed content/layout. No new UI framework,
  fonts, textures, crates or renderer features. Input modal gating precedes camera.

GitHub API authentication currently returns 401; public web reading and a fetch
without the broken credential configuration work. Merge status and complete
issue #17 were verified via the GitHub web payload. Writing/CI access must be
restored before final publication; no credentials are stored in this repository.
