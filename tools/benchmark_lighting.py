#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Compare the lighting room with Slice 19 using cached dependencies, offline.
Baseline has the exact same shell/camera but no unsupported Torch variant. Only
its dark row is a matched no-source comparison; other rows measure new capability.
"""
import argparse
import csv
import io
import statistics
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent

def run(command, **kwargs):
    return subprocess.check_output(command, cwd=ROOT, text=True, **kwargs)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', default='9eec01272d463c6eaa350bcbf3345d24eba6b53f')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    scratch = ROOT / 'work'
    scratch.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='lighting-baseline-', dir=scratch) as temp:
        baseline = Path(temp)
        paths = run(['git', 'ls-tree', '-r', '--name-only', args.baseline, 'crates/voxel_core/src']).splitlines()
        for path in paths:
            dest = baseline / Path(path).relative_to('crates/voxel_core')
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(subprocess.check_output(['git', 'show', f'{args.baseline}:{path}'], cwd=ROOT))
        (baseline / 'Cargo.toml').write_text('[package]\nname="voxel_core"\nversion="0.1.0"\nedition="2024"\n[workspace]\n[dependencies]\ncrc32fast="1.5.0"\n')
        (baseline / 'examples/support').mkdir(parents=True)
        source = ROOT / 'crates/voxel_core/examples'
        fixture = (source / 'support/lighting_fixture.rs').read_text().replace('Block::Torch', 'Block::Air')
        (baseline / 'examples/support/lighting_fixture.rs').write_text(fixture)
        bench = (source / 'lighting_bench.rs').read_text().replace('Block::Torch', 'Block::Air').replace('light.bytes()', 'light.cells()').replace('light.block_propagated', '0')
        (baseline / 'examples/lighting_bench.rs').write_text(bench)
        # Finish compilation before either measurement; run each process alone.
        subprocess.run(['cargo', 'build', '--locked', '--offline', '--release', '-p', 'voxel_core', '--example', 'lighting_bench'], cwd=ROOT, check=True)
        subprocess.run(['cargo', 'build', '--offline', '--release', '--manifest-path', str(baseline / 'Cargo.toml'), '--example', 'lighting_bench'], cwd=ROOT, check=True)
        executables = {'before': baseline / 'target/release/examples/lighting_bench', 'after': ROOT / 'target/release/examples/lighting_bench'}
        measurements = {'before': [], 'after': []}
        for iteration in range(3):
            for name in (['before', 'after'] if iteration % 2 == 0 else ['after', 'before']):
                raw = run([str(executables[name])])
                (args.output / f'{name}-{iteration+1}.csv').write_text(raw)
                measurements[name].append(list(csv.DictReader(io.StringIO(raw))))
        for name, runs in measurements.items():
            with (args.output / f'{name}.csv').open('w', newline='') as output:
                writer = csv.DictWriter(output, fieldnames=runs[0][0].keys(), lineterminator='\n')
                writer.writeheader()
                for i, row in enumerate(runs[0]):
                    row = dict(row)
                    for field in ['light_us', 'mesh_us']:
                        row[field] = f'{statistics.median(float(r[i][field]) for r in runs):.2f}'
                    writer.writerow(row)
    (args.output / 'environment.txt').write_text(platform.platform()+'\n'+run(['rustc', '--version'])+'Baseline '+args.baseline+'\nCPU-only; no GPU/frame-time claim.\n')
    print((args.output / 'before.csv').read_text())
    print((args.output / 'after.csv').read_text())
