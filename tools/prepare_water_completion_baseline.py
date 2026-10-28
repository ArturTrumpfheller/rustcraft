#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Export PR39's exact pre-completion baseline and inject the matched CPU harness.

The original water_gallery is already present; do not disable its atmosphere.
No gameplay algorithm is changed in the baseline. New-only pickup metrics are
omitted because the baseline has no item ownership/pickup API.
"""
import io
from pathlib import Path
import subprocess
import sys
import tarfile

root = Path(__file__).resolve().parents[1]
destination = Path(sys.argv[1]).resolve()
if destination.exists():
    raise SystemExit(f"Refusing to overwrite {destination}")
destination.mkdir(parents=True)
archive = subprocess.check_output(["git", "archive", "a976d37b9bc949ba3e283df39e1236812ab74c69"], cwd=root)
with tarfile.open(fileobj=io.BytesIO(archive)) as source:
    source.extractall(destination, filter="data")
manifest = destination / "Cargo.toml"
manifest.write_text(manifest.read_text().replace('version = "0.1.0"', 'version = "0.1.22"', 1))
lock = destination / "Cargo.lock"
text = lock.read_text()
for package in ["rustcraft", "voxel_core"]:
    text = text.replace(f'name = "{package}"\nversion = "0.1.0"', f'name = "{package}"\nversion = "0.1.22"')
lock.write_text(text)
relative = "crates/voxel_core/examples/water_completion_bench.rs"
text = (root / relative).read_text()
start = text.index("    // COMPLETION_ONLY_BEGIN")
end = text.index("    // COMPLETION_ONLY_END") + len("    // COMPLETION_ONLY_END")
(destination / relative).write_text(text[:start] + text[end:])
print(destination)
