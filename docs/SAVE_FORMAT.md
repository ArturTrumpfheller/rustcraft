# RustCraft world snapshot version 1

Project-authored specification: MIT OR Apache-2.0. Implemented by `voxel_core::persistence`, independent of Bevy. One local world is stored in `world.rcw`; `world.rcw.bak` is its previous valid generation. Paths and recovery commands are in [README](../README.md#saving-loading-and-recovery).

## Byte contract

All integers are little-endian, with no padding. Coordinates are signed two's-complement i32. This format does not use Rust struct, enum, chunk-vector or ECS memory layouts.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | ASCII magic `RUSTWRLD` |
| 8 | 4 | u32 format version, exactly 1 |
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
| 3 | Stone |

Every record must be within X/Z [-32,32), Y [-16,32). Duplicate/unsorted coordinates, ID 0 or unknown IDs, invalid magic/version, inconsistent counts/lengths, checksum errors, truncation and trailing bytes are rejected. The maximum is 196,608 records and **2,752,536 bytes**, enforced before disk reads allocate an unbounded buffer. This finite-volume limit is part of version 1. Trusted imports outside the volume cannot be saved, including out-of-bounds empty chunks.

A snapshot is the entire world, sparsely encoded by non-air content. Missing coordinates are Air, not a reference to demo terrain. This preserves both removed and replaced original terrain, negative/boundary blocks and new chunks. Empty chunks need not be stored. A valid 24-byte empty snapshot decodes to an empty world. Only absence of both primary and backup creates the original demo. There is no persisted player state, seed, inventory or presentation data.

An independent one-Stone-at-(0,0,0) golden fixture is pinned in tests. Encoding is canonical: repeating a load/encode produces identical bytes regardless of transient revisions or insertion order. CRC detects accidental damage, not malicious alteration or authenticity.

## Upgrade policy

Version 1 is deliberately small and generator-independent. A new demo or future seeded generator must not regenerate terrain over an existing snapshot. New meanings, dimensions, coordinates outside these limits or a block registry need a new documented version and explicit migration. IDs must never be reassigned. Unknown versions/IDs currently stop loading without rewriting files; there is no speculative migration layer. Back up real worlds before future upgrades.

## Commit and failure policy

`SaveStore` holds an exclusive OS lock on `world.lock` until the session and its worker finish. Another cooperating RustCraft instance cannot open the same directory. Do not remove/replace the lock file while a process runs. Locks are released by the OS on process exit; the empty lock file may remain. This targets local desktop filesystems, not distributed/network-filesystem coordination.

1. Encode a bounded immutable world snapshot on the save worker.
2. Read and validate any existing primary. Refuse to replace corrupt/unknown data. If the primary vanished but a backup exists, refuse and request explicit recovery.
3. If a valid primary exists, copy its bytes to an exclusively created temporary file beside `world.rcw.bak`, flush and `sync_all`, close it, then rename over the backup. On Unix, sync the directory as well. Do not advance to the new primary if backup creation fails.
4. Write the new snapshot to an exclusively created same-directory temporary file beside `world.rcw`, flush and `sync_all`, close it, then rename over the primary. On Unix, sync the directory. Report success only after all required operations return successfully.

The implementation uses Rust `std::fs::rename` with replacement semantics on Linux and Windows and closes all read/temp handles before replacement. It never opens the primary for truncation or deletes it before rename. Temporary names include process ID and a counter, with collision retries; errors clean up only the temporary file this operation owns. Crash leftovers are ignored on load and can be removed after all instances exit.

| Failure / state | Behavior |
| --- | --- |
| Missing primary and backup | Clean demo; no world file written until a meaningful change |
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

F5 while focused requests saving if dirty. Every 30 seconds of real application time, autosave checks for meaningful changes, including while player simulation is paused or unfocused. Clean frames, failed/no-op edits and unchanged loaded/demo worlds do not write. A change followed by a revert still counts as changed; revisions intentionally do not hash the whole world every frame.

Only the small bounded world clone happens on the main thread; all format and filesystem work is on one save thread. Edits may continue while it runs. Success marks only the captured revision clean; newer changes remain dirty. Manual requests queue behind a current job. A native close pauses controls, waits for a current job and saves any newer changes, then exits. Failures keep dirty data available for retry and are logged. The title shows `New demo`, `Loaded`, `Dirty`, `Saving`, `Saved` or `ERROR / DIRTY`. Startup logs always show the actual save path and source.

Tests use isolated temporary directories, independent format fixtures, injected partial-write/commit failures, real backup-path errors, restart tests and actual ECS scheduling. See [validation](VALIDATION.md) for executed platform and GUI coverage.
