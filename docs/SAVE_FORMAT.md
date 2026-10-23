# RustCraft saves — v1/v2 snapshots, v3 finite overlays and v4 regional worlds

Project-authored specification: MIT OR Apache-2.0. Implemented by `voxel_core::persistence`, independent of Bevy. The root `world.rcw` is a complete snapshot/overlay in v1–v3, or an immutable identity manifest in v4. Regional v4 data lives beside it under `regions/`. Backups retain one previous generation of each replaced data file. Paths and recovery commands are in [README](../README.md#saving-loading-and-recovery).

## Slice14 catalog envelope

The following binary world/player formats are unchanged. Their complete bundle
now lives in `<data-root>/worlds/<catalog-ID>/`. Independent checksummed
`world.meta` stores the display name and catalog creation/import date. Its exact
layout, lock/staging/archive rules and non-destructive legacy import are specified
in [WORLDS](WORLDS.md). A copied legacy identity is preserved, never rekeyed. Keep
all regions, sidecars, journals and backups together when copying/restoring.

## Legacy byte contract (v1/v2)

All integers are little-endian, with no padding. Coordinates are signed two's-complement i32. This format does not use Rust struct, enum, chunk-vector or ECS memory layouts.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | ASCII magic `RUSTWRLD` |
| 8 | 4 | u32 format version: 1 (legacy read) or 2 (legacy write) |
| 12 | 4 | u32 record count N |
| 16 | 4 | u32 payload length, exactly 14 × N |
| 20 | 4 | u32 CRC-32/ISO-HDLC (the standard zlib/IEEE CRC-32) |
| 24 | 14 × N | N records, strictly increasing lexicographic signed (x,y,z) order |

The CRC covers header bytes `[0,20)` followed by the entire payload `[24,end)`, excluding the checksum itself. Each 14-byte record consists of i32 X, i32 Y, i32 Z, u16 block ID. Stable material mapping:

| ID | Meaning |
| --- | --- |
| 0 | Air, implicit through absence; never stored as a record |
| 1 | Grass |
| 2 | Dirt |
| 3 | Stone, v1 and v2 |
| 4 | Wood, v2 only |
| 5 | Leaves, v2 only |

IDs 1 and 2 also retain their exact v1 meaning. Version 1 strictly rejects IDs 4/5; version 2 accepts 1–5. Both new materials are solid and opaque. Air is implicit in both versions.

Every record must be within X/Z [-32,32), Y [-16,32). Duplicate/unsorted coordinates, ID 0 or unknown IDs, invalid magic/version, inconsistent counts/lengths, checksum errors, truncation and trailing bytes are rejected. The maximum is 196,608 records and **2,752,536 bytes**, enforced before disk reads allocate an unbounded buffer. This finite-volume limit is part of both versions. Trusted imports outside the volume cannot be saved, including out-of-bounds empty chunks.

A snapshot is the entire world, sparsely encoded by non-air content. Missing coordinates are Air, not a reference to demo terrain. This preserves both removed and replaced original terrain, negative/boundary blocks and new chunks. Empty chunks need not be stored. A valid 24-byte empty snapshot decodes to an empty world. Only absence of both primary and backup invokes the new-world factory. Slice 6 created v3 streamed identity/overlay worlds; Slice 7 creates v4 regional worlds; the library’s legacy `open` helper retains the historical demo factory. Neither factory is called for a loaded/corrupt/unsupported file. There is no persisted player state, seed, inventory or presentation data. Legacy terrain is saved in full; no generator is used to restore it. This paragraph describes v1/v2 only.

Independent v1 golden fixtures (one Stone and mixed materials/negative coordinates) are pinned in tests; re-encoding uses v2 while retaining exact block semantics. Encoding is canonical: repeating a load/encode produces identical bytes regardless of transient revisions or insertion order. CRC detects accidental damage, not malicious alteration or authenticity.

## Legacy upgrade / mode policy

v1 and v2 share the header/record/CRC layout and full-snapshot semantics. **Loading never writes or migrates a file.** A valid v1 primary (including removed terrain or a completely empty world) is authoritative. The seed override is not parsed for any existing save. v1/v2 stays in explicitly labelled legacy finite mode. Loaded worlds start clean; idle autosave and F5 with no changes leave the original bytes untouched.

After meaningful edits, the normal F5/autosave/final-save protocol validates the old v1 primary, backs its exact bytes up, and commits a complete v2 snapshot. Even if only old materials remain, current saves use v2. A failed upgrade follows the same recovery policy as any save and never truncates the old primary. The ordinary backup retains only one generation: a subsequent save may replace that legacy backup with v2. Keep an external directory copy if rollback to an older client is important. Older v1-only clients safely reject v2; there is no automatic downgrade.

Version 2 adds only explicit IDs 4/5, not metadata, larger bounds or changed terrain defaults. There is **no implicit v1/v2-to-v3 migration**. The store refuses changing identity/mode/seed when replacing an existing primary. IDs must never be reassigned. Future dimensions, bounds or material semantics require another version and an explicit migration policy. Unknown versions/IDs stop loading without rewriting files. Seeded generation changes only truly new worlds, never existing snapshots.

## Streamed overlay byte contract (v3)

v3 is a separate procedural world mode; the resident cache is never serialized as if it were a full snapshot. All integers remain little-endian. It reuses the first 24 bytes (magic, version=3, record count, payload length=14×N and CRC). Additional metadata:

| Offset | Size | Field |
| --- | --- | --- |
| 24 | 8 | u64 world seed |
| 32 | 4 | u32 generator version, currently exactly 2 |
| 36 | 4 | u32 reserved, must be 0 |
| 40 | 14 × N | Sorted unique i32 X/Y/Z + u16 block-ID overlay records |

CRC-32 covers bytes `[0,20)` and `[24,end)`, including seed/version/reserved metadata and payload. Supported IDs are 0=Air, 1=Grass, 2=Dirt, 3=Stone, 4=Wood, 5=Leaves. **ID 0 is explicit and valid only in v3**, retaining removed generated terrain. All records must be within X/Z [-64,64), Y [-16,32). At most 786,432 records / **11,010,088 bytes** are allowed. Bounded disk reads reject files larger than the largest supported mode; each decoder additionally applies its own mode-specific limits.

Absent overlay entries mean the frozen generator-v2 block at that coordinate; an explicit Air entry means removed terrain. No ECS data, meshes, resident/visited set, player state or queue is stored. An unedited v3 world is exactly 40 bytes (identity + zero records). Loading creates an empty resident cache plus the complete authoritative overlay. The spawn cache is prepared from the persisted identity; other columns generate on demand. An environment seed never replaces the stored seed.

Serialization includes **all overlay entries**, including edits in evicted chunks, not only resident cells. Cache installs/evictions do not change edit revision. Local mesh dependency tokens are transient scheduling data, distinct from the global save revision; neither tokens nor respawn/loading state enter the file. Immutable snapshots share the edit map; subsequent edits use copy-on-write. A successful older revision cannot mark newer edits clean. Reverting a cell can leave an explicit redundant entry; compaction is intentionally deferred.

Unknown save/generator versions, nonzero reserved fields, bad CRC/count/length/order/IDs/coordinates stop loading before generation or replacement. Version 3 requires generator v2; generator v1 is retained as a reference but is not a streamed identity. Generator-v2 output is pinned by golden fixtures and must not be retuned in place. Future larger bounds/identities require an explicit format/version decision.

## Commit and failure policy

`SaveStore` holds an exclusive OS lock on `world.lock` until the session and its worker finish. Another cooperating RustCraft instance cannot open the same directory. Do not remove/replace the lock file while a process runs. Locks are released by the OS on process exit; the empty lock file may remain. This targets local desktop filesystems, not distributed/network-filesystem coordination.

1. Encode a bounded immutable full legacy snapshot or complete v3 identity/overlay on the save worker.
2. Read and validate any existing primary. Refuse to replace corrupt/unknown data or change world identity/mode. If the primary vanished but a backup exists, refuse and request explicit recovery.
3. If a valid primary exists, copy its bytes to an exclusively created temporary file beside `world.rcw.bak`, flush and `sync_all`, close it, then rename over the backup. On Unix, sync the directory as well. Do not advance to the new primary if backup creation fails.
4. Write the new snapshot to an exclusively created same-directory temporary file beside `world.rcw`, flush and `sync_all`, close it, then rename over the primary. On Unix, sync the directory. Report success only after all required operations return successfully.

The implementation uses Rust `std::fs::rename` with replacement semantics on Linux and Windows and closes all read/temp handles before replacement. It never opens the primary for truncation or deletes it before rename. Temporary names include process ID and a counter, with collision retries; errors clean up only the temporary file this operation owns. Crash leftovers are ignored on load and can be removed after all instances exit.

| Failure / state | Behavior |
| --- | --- |
| Missing primary and backup | Current client creates a v4 manifest (see regional protocol below); Slice 6 created v3 identity/empty overlay once, even without edits |
| Missing primary with existing backup | Stop; explicit backup recovery required |
| Corrupt, truncated, unknown version/ID or unreadable primary | Stop before the game starts; preserve all files |
| Lock or directory unavailable | Actionable startup error; no fallback to another save location |
| Temporary write/flush/sync/backup failure | Keep previous primary; remain dirty; show failure |
| Failure before primary rename | Previous primary remains usable; a valid backup may also exist |
| Successful replacement | Primary is new world; backup is the previous world |
| Failure after rename (e.g. Unix directory sync) | Report failure, though the new primary may already be present; never falsely report success |
| Final-save failure | Keep window open, cancel close, retain dirty world for F5 retry |

A first save has no previous generation to back up. Recovery never silently discards newer data: with the game closed, copy the entire directory, preserve any damaged primary and copy a validated backup to `world.rcw`. The next startup validates it normally. A backup can be one or more edit intervals behind. A retry after an ambiguous post-rename failure may rotate the newly committed primary into the backup; preserve files externally before diagnosing such failures.

This protocol protects the known good primary against ordinary partial writes and common I/O failures. File/directory sync and replacement guarantees depend on the filesystem, OS and hardware. Windows uses synced files plus rename but does not perform a Unix-style directory fsync. There is no absolute power-loss guarantee on either platform. Force-kill/crash can lose changes since the last completed save; a normal native window close uses the save-before-exit protocol. External programs that mutate files while the game runs are outside the locking contract. Do not use a live data directory with syncing/backup software that replaces its files.

## Client scheduling and status

F5 while focused requests saving if dirty. Every 30 seconds of real application time, autosave checks for meaningful changes, including while player simulation is paused or unfocused. Clean frames, failed/no-op edits and unchanged loaded worlds do not write. New generated worlds have a separate first-creation pending flag: the first Update requests a background save, even if the world has never been edited. Failure retains this flag and dirty status; successful completion clears it once. Later edits during that job remain dirty through revision tracking. A change followed by a revert still counts as changed; revisions intentionally do not hash the whole world every frame.

Legacy saves clone bounded resident data; v3 snapshots capture only identity/revision and the immutable overlay Arc, excluding resident chunk vectors; all format and filesystem work is on one save thread. Edits may continue while it runs. Success marks only the captured revision clean; newer changes remain dirty. Manual requests queue behind a current job. A native close pauses controls, waits for a current job and saves any newer changes, then exits. Failures keep dirty data available for retry and are logged. Unsaved v3 edits survive cache eviction in memory but are durable only after a successful F5/autosave/final save. There is no per-click crash journal; a forced kill can lose edits since the last completed save. Failed saves retain the complete dirty overlay even while cache eviction continues.

The in-game bitmap panel shows loaded/unsaved state, yellow `SAVING...`, green `SAVED`, or persistent red `SAVE ERROR - F5 RETRY`, even with the cursor released. Title/logs retain status too. Startup logs always show the actual save path and source.

Tests use isolated temporary directories, independent format fixtures, injected partial-write/commit failures, real backup-path errors, restart tests and actual ECS scheduling. See [validation](VALIDATION.md) for executed platform and GUI coverage.

## v4 manifest and regional overlay contract (Slice 7)

Only genuinely new worlds select v4 / generator v3. **No v1/v2/v3 world is
implicitly migrated or expanded.** Their byte contracts, bounds and generators
remain unchanged. A root manifest is exactly 64 bytes, with little-endian fields:

| Offset | Bytes | Field |
| --- | --- | --- |
| 0 | 8 | `RUSTWRLD` |
| 8 | 4 | World format 4 |
| 12 | 8 | Record count and payload length, both 0 |
| 20 | 4 | CRC-32 over [0,20) + [24,64) |
| 24 | 8 | u64 seed |
| 32 | 4 | Generator 3 |
| 36 | 4 | Reserved 0 |
| 40 | 4 | Supported X/Z half-extent 32768 |
| 44 | 16 | Nonzero world nonce |
| 60 | 4 | Reserved 0 |

The nonce is a world identifier, not a cryptographic credential. The generator,
extent and reserved fields are strict; future extent/algorithm changes need a
new explicit version decision. `encode`/`decode` on a v4 world handle this manifest
only; **use SaveStore.save for regional data**, never encode a resident cache as a
complete save. `save_snapshot` retains dirty region maps, not resident chunks.

A region covers 64×64 blocks and the full Y [-16,32) range. Signed region (rx,rz)
is floor-divided from world X/Z by 64; block -1 belongs to region -1. The path is
`regions/<rx div_euclid 32>_<rz div_euclid 32>/<rx>_<rz>.rcw`. At most 1,024 primary
region coordinates share a shard; directories/files are created only on edits.
There is no startup scan/global visited-region index. A never-edited region has
no file. A missing primary with a backup is a recovery error, not empty terrain.
External deletion of a sole primary cannot be distinguished from a never-edited
region; external mutation is outside the session locking contract. Keep whole-
directory backups with the game closed.

Each region file has its own frozen version, independent of world-format v1:

| Offset | Bytes | Field |
| --- | --- | --- |
| 0 | 8 | `RUSTREGN` |
| 8 | 4 | Region format 1 |
| 12 | 4 | Record count N, at most 196,608 |
| 16 | 4 | Payload length, exactly 14×N |
| 20 | 4 | CRC-32 over [0,20) + [24,end) |
| 24 | 16 | Matching world nonce |
| 40 | 8 | Matching seed |
| 48 | 4 | Generator 3 |
| 52 | 4 | Signed i32 rx |
| 56 | 4 | Signed i32 rz |
| 60 | 4 | Reserved 0 |
| 64 | 14×N | Sorted unique global i32 X/Y/Z + u16 ID records |

IDs 0–5 have their established meanings; explicit Air=0 preserves removals.
Absent records refer to the frozen generator-v3 cell. Every coordinate must belong
to the named valid region and vertical range. Unknown IDs/versions, mismatched
identity/region, bad CRC, truncation, trailing/duplicate/unsorted/out-of-range data
are rejected. Maximum file size is **2,752,576 bytes**, enforced before reading.
One edited cell uses a 78-byte region plus the 64-byte manifest; two use 92 bytes.

The manifest is committed/synced before first region writes and remains unchanged
on ordinary saves. Each dirty region independently reuses validated previous data,
synced same-directory temporary files, backup replacement and primary rename.
New shard directory entries are synced on Unix. `.rcw.bak` retains the previous
valid version of that region. Identical retries retain that backup and repeat
needed directory sync. There is no whole-world transaction: a failed save can
commit some regions and leave others old. All dirty in-memory regions remain
pinned for retry; only complete successful captured revisions are marked clean.
A failed final save cancels close. Later edits during a save remain dirty.

At most eight dirty regions may accumulate; four request a pressure save in
addition to F5 / 30-second autosave / final close. When that hard limit is reached,
edits in another clean region are rejected until a successful save frees capacity.
Dirty voxel/GPU chunks can still unload: their regional overlay stays in RAM.
This bounds memory during disk errors without deleting changes. Regional COW
cost is bounded by one region, not every edit ever made. Disk-full and permission
errors remain visible; there is no fallback to another world/data directory.

Corrupt/missing/unknown regional primaries are never automatically replaced by
generated terrain or a backup. Spawn-region errors stop startup; later load errors
show `LOAD ERROR - F5 RETRY` and leave solid pending barriers. After preserving the
entire directory, recover the matching region backup explicitly while the game is
closed, then restart/retry. A missing/corrupt root manifest needs its matching
external backup; v4 does not rotate an unchanged manifest on every regional save.
An orphan `regions` directory without a manifest prevents new identity creation.
Copying only `world.rcw` is **not a backup of a v4 world**. File-level atomicity,
Windows directory-sync limitations and losses since the last successful save on
forced termination remain as documented above. No per-edit crash journal exists.

## Player/session v1 sidecars (Slice 9; world v1–v4 unchanged)

The preceding world contracts remain byte-for-byte unchanged. Production clients
now use `load_player` before spawn and `save_player_session` for all saves. The
world-only `save` API refuses inventory-enabled directories. Initializing an
inventory on legacy v1 does not upgrade or replace the world file; an actual
world edit retains the existing v1→v2 policy. No generators/IDs are reassigned.

All sidecar integers are little-endian. Common header:

| Offset | Bytes | Meaning |
| --- | --- | --- |
| 0 | 8 | File magic (below) |
| 8 | 4 | Sidecar version, exactly 1 |
| 12 | 4 | Count (36 for player, 0 for identity/journal) |
| 16 | 4 | Exact payload byte length |
| 20 | 4 | CRC-32 over [0,20) + [24,end), excluding checksum field |

`player-world.rcid`: **56 bytes**, magic `RUSTPID1`, payload a 32-byte immutable
binding: class u32 (2 = finite v1/v2; 3 = v3; 4 = v4), seed u64, generator u32,
nonce [u8;16]. Finite class requires seed/gen zero; v3 requires its frozen seed/gen;
v4 requires seed/gen3/nonce from manifest. Nonce must be nonzero; legacy modes
receive a time/process/counter-generated sidecar identity, not cryptographic RNG.

`player.rcp`: **176 bytes**, magic `RUSTPLY1`, count 36, payload 152:

| Offset | Bytes | Meaning |
| --- | --- | --- |
| 24 | 32 | Exact immutable binding |
| 56 | 8 | Root fingerprint: u32 complete root-file length + u32 CRC-32 of root bytes |
| 64 | 1 | Selected zero-based hotbar index, 0–8 |
| 65 | 108 | 36 ordered slots: hotbar 0–8, then three row-major main rows |
| 173 | 3 | Held cursor stack |

Each three-byte stack is u16 item ID plus u8 count. `(0,0)` is empty; only IDs
1 Grass, 2 Dirt, 3 Stone, 4 Wood, 5 Leaves with counts **1–64** are valid. ID0
with nonzero count, nonzero ID with zero count, unknown IDs and oversized stacks
are rejected. Inventory revision and modal-open state are not saved. A held stack
opens inventory on load. Player position/view/velocity remain outside this format.

`session.pending`: magic `RUSTTXN1`, count0, maximum **32 MiB total**, exact ordered
payload:

1. world-dirty u8, only 0 or 1;
2. u32 root length + complete intended root file (legacy full snapshot, v3 overlay
   or v4 manifest), validated with the existing decoder;
3. u32 regional entry count, at most eight; each entry is signed i32 rx, i32 rz,
   u32 byte length + complete encoded v4 region file. Positions must be strictly
   ordered/unique, identity and coordinates valid. Clean-world journals have none;
4. u32 player length + complete 176-byte player file with matching binding and
   fingerprint of the embedded root; no trailing bytes.

Files are bounded before reading; CRC, versions, lengths, enum/counts, nested
world/regions and identity are all validated. No automatic fallback, regeneration
or starter refill replaces corrupt/unknown/foreign/missing initialized player data.
A missing primary with player backup/identity prevents reinitialization. Orphan
player files also prevent a missing world from silently invoking a new factory.

### Paired commit and recovery

Within the existing OS session lock, one worker captures world and inventory,
atomically replaces/syncs `session.pending`, validates an existing player primary
before any world advancement, writes changed world files with their existing
validated backup/replace protocol, then backs up/replaces/syncs player. Finally it
removes the journal and syncs the directory on Unix. Only full success marks the
captured revisions clean; newer RAM changes remain dirty. F5, autosave, pressure
save and normal close all include inventory-only changes. Failed final save keeps
the application open. Idempotent retries avoid rotating unchanged world/player
backups away. `player.rcp.bak` is the previous valid player, not a session rollback.

A known valid pending journal replays idempotently **before gameplay**, completing
all captured dirty regions and player. Missing initial root can be recovered from
valid pending intent without invoking another world/starter factory. Unknown/bad
journal, corrupt current player or identity mismatch stops loading and preserves
files. Multi-file writes are not simultaneous; the journal provides **save-level
redo**, not per-action durability. Changes after the captured save can be lost on
forced termination. This extends the historical per-file protocol above; it is
not a claim that individual region/backup files form an atomic session.

Keep complete backups with the game closed, including root, all regions, player,
identity and pending journal. To recover, preserve the entire damaged directory
and restore a complete consistent copy. Never restore player alone or delete a
journal to make an error disappear. The v4 root fingerprint cannot detect all
historical region replacements: external edits and old clients ignoring sidecars
can break consistency. Use older clients only on isolated copies.

Initial creation commits identity before preparing the first journal. Failure in
that narrow gap leaves an identity without player/journal and deliberately fails
closed; there is no guessed refill. Preserve existing data and recover a complete
backup; for a genuinely disposable fresh test use another fresh directory.
Windows directory-sync and filesystem/hardware limits described above still apply;
checksums/nonce are validation, not cryptographic authenticity. Dense captures can
require tens of MiB of bounded temporary encoding and journal disk I/O. See
INVENTORY and VALIDATION for behavior, injected-failure tests and measurements.

## Derived skylight (Slice 10)

Skylight/face colors, pristine generation cache, chunk light samples and dependency
tokens are reconstructed from authoritative blocks and overlays. They are never
serialized into world v1–v4, regions, player/identity or session journal files.
Initial inventory, paired saving/replay, checksums and recovery rules above remain
unchanged. Lighting cannot grant/consume items or change blocks. Tests compare
legacy/v3/v4 bytes and reconstruct identical edited-world lighting after paired
v1/v2/v3/v4 save/restart, including distant regional edits. See LIGHTING/VALIDATION.
