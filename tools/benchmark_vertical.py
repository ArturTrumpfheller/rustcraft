#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Run the identical release harness on a detached Slice25 worktree and this checkout."""
import argparse
import os
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--scratch', type=Path, default=Path('work/vertical-benchmark'))
    parser.add_argument('--repeats', type=int, default=3)
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error('--repeats must be positive')
    root = Path(__file__).resolve().parents[1]
    baseline, output, scratch = (p.resolve() for p in (args.baseline, args.output, args.scratch))
    if baseline == root:
        parser.error('baseline must be a separate detached worktree')
    source = root / 'crates/voxel_core/examples/vertical_bench.rs'
    destination = baseline / source.relative_to(root)
    original = source.read_text()
    # Measurement-only adapters for APIs introduced by Slice26. No baseline
    # production code changes; historical chunks always allocate4096 cells.
    substitutions = {
        'let nonempty = w.nonempty_chunk_count();': 'let nonempty = w.chunks().filter(|(_,c)| (0..16).any(|y| (0..16).any(|z| (0..16).any(|x| c.get(voxel_core::LocalPos::new([x,y,z]).unwrap()) != Block::Air)))).count();',
        'let voxels = w.allocated_voxel_bytes();': 'let voxels = w.chunks().count()*4096;',
        'let cache_bytes = cache.allocated_voxel_bytes();': 'let cache_bytes = cache.columns()*3*4096;',
        'cache.allocated_voxel_bytes()': 'cache.columns()*3*4096',
    }
    adapted = original
    for before, after in substitutions.items():
        if before not in adapted:
            raise RuntimeError(f'harness adapter no longer matches: {before}')
        adapted = adapted.replace(before, after)
    if destination.exists() and destination.read_text() != adapted:
        parser.error(f'refusing to overwrite another harness: {destination}')
    destination.write_text(adapted)
    output.mkdir(parents=True, exist_ok=True)
    builds = {}
    for name, cwd in [('baseline', baseline), ('current', root)]:
        target = scratch / name
        subprocess.run(['cargo', 'build', '--release', '--locked', '--target-dir', str(target), '-p', 'voxel_core', '--example', 'vertical_bench'], cwd=cwd, check=True)
        builds[name] = target / ('release/examples/vertical_bench' + ('.exe' if os.name == 'nt' else ''))
    for name, executable, version in [('baseline-v5', builds['baseline'], 5), ('current-v5', builds['current'], 5), ('current-v6', builds['current'], 6)]:
        for repeat in range(1, args.repeats + 1):
            path = output / f'{name}-{repeat}.txt'
            with path.open('x') as stream:
                subprocess.run([str(executable), str(version)], stdout=stream, cwd=root, check=True)
            print(path, flush=True)


if __name__ == '__main__':
    main()
