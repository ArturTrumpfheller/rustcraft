#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Compare Slice23's actual source with Slice24 using the identical release harness."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BASE = '7752287'

def run(command, **kwargs):
    return subprocess.run(command, cwd=ROOT, check=True, **kwargs)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--samples', type=int, default=3)
    args = parser.parse_args()
    assert 1 <= args.samples <= 7
    args.output.mkdir(parents=True, exist_ok=False)
    scratch = ROOT / 'work'
    scratch.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='terrain-baseline-', dir=scratch) as directory:
        baseline = Path(directory)
        paths = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', BASE, 'crates/voxel_core/src'], cwd=ROOT, text=True).splitlines()
        for name in paths:
            dest = baseline / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(subprocess.check_output(['git', 'show', f'{BASE}:{name}'], cwd=ROOT))
        core = baseline / 'crates/voxel_core'
        (core / 'Cargo.toml').write_text('[package]\nname="voxel_core"\nversion="0.1.0"\nedition="2024"\n[workspace]\n[dependencies]\ncrc32fast="=1.5.3"\n')
        (core / 'examples').mkdir()
        shutil.copyfile(ROOT / 'crates/voxel_core/examples/terrain_bench.rs', core / 'examples/terrain_bench.rs')
        run(['cargo', 'build', '--offline', '--release', '--manifest-path', str(core / 'Cargo.toml'), '--example', 'terrain_bench'])
        run(['cargo', 'build', '--locked', '--release', '-p', 'voxel_core', '--example', 'terrain_bench'])
        for sample in range(1, args.samples + 1):
            cases = [('slice23', core / 'target/release/examples/terrain_bench', '3'), ('slice24', ROOT / 'target/release/examples/terrain_bench', '4')]
            for name, executable, version in cases if sample % 2 else reversed(cases):
                with (args.output / f'{name}-{sample}.csv').open('w') as output:
                    run([str(executable), version], stdout=output)
                print(f'{name} sample {sample}', flush=True)
        (args.output / 'environment.txt').write_text(subprocess.check_output(['rustc', '--version'], text=True) + subprocess.check_output(['uname', '-srmo'], text=True) + f'baseline={BASE}\nCARGO_BUILD_JOBS={os.environ.get("CARGO_BUILD_JOBS", "default")}\nRequested heap high-water; not process RSS, native GPU or main-thread frame latency.\n')
