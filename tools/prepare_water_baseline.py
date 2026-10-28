#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Export accepted Slice21 and inject the identical dry renderer fixture.

Usage: python tools/prepare_water_baseline.py work/water-baseline
Then build its water_gallery with CARGO_TARGET_DIR pointing at the main target,
and copy the binary before building Slice22. Never use --baseline: both phases
must retain the accepted atmosphere shader.
"""
from pathlib import Path
import subprocess
import sys
import tarfile
import io

root = Path(__file__).resolve().parents[1]
destination = Path(sys.argv[1]).resolve()
if destination.exists():
    raise SystemExit(f'Refusing to overwrite {destination}')
destination.mkdir(parents=True)
archive = subprocess.check_output(['git','archive','91bce77'],cwd=root)
with tarfile.open(fileobj=io.BytesIO(archive)) as source:
    source.extractall(destination,filter='data')
# Keep path-dependency artifact identities distinct when sharing a Cargo target.
manifest = destination/'Cargo.toml'
manifest.write_text(manifest.read_text().replace('version = "0.1.0"', 'version = "0.1.21"', 1))
lock=destination/'Cargo.lock'
lock_text=lock.read_text()
for package in ['rustcraft','voxel_core']:
    lock_text=lock_text.replace(f'name = "{package}"\nversion = "0.1.0"',f'name = "{package}"\nversion = "0.1.21"')
lock.write_text(lock_text)
for relative in ['client/examples/water_gallery.rs','crates/voxel_core/examples/support/water_fixture.rs']:
    lines=[]
    skip=False
    for line in (root/relative).read_text().splitlines():
        if '// SLICE22_BEGIN' in line:
            skip=True
        elif '// SLICE22_END' in line:
            skip=False
        elif not skip:
            lines.append(line)
    (destination/relative).write_text('\n'.join(lines)+'\n')
print(destination)
