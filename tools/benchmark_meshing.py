#!/usr/bin/env python3
"""Compare the current mesher with a local Git baseline, without another checkout.
Requires the pinned Rust toolchain and cached core dependencies (cargo --offline).
"""
import argparse
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--baseline', default='535c636')
parser.add_argument('--output', type=pathlib.Path, required=True)
args = parser.parse_args()
# Resolve once, so neither source reads nor the report can race a branch move.
revision = subprocess.check_output(['git', 'rev-parse', '--verify', args.baseline + '^{commit}'], cwd=ROOT, text=True).strip()
args.output.mkdir(parents=True, exist_ok=False)
(args.output / 'baseline.txt').write_text(revision + '\n', encoding='utf-8')
with tempfile.TemporaryDirectory(prefix='rustcraft-meshing-') as tmp:
    baseline = pathlib.Path(tmp)
    prefix = 'crates/voxel_core/'
    files = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', revision, prefix + 'src/'], cwd=ROOT, text=True).splitlines()
    for name in files:
        target = baseline / name.removeprefix(prefix)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(subprocess.check_output(['git', 'show', revision + ':' + name], cwd=ROOT))
    (baseline / 'Cargo.toml').write_text('[workspace]\n[package]\nname="voxel_core"\nversion="0.1.0"\nedition="2024"\n[dependencies]\ncrc32fast="=1.5.3"\n', encoding='utf-8')
    harness = (ROOT / prefix / 'examples/meshing_bench.rs').read_text(encoding='utf-8')
    (baseline / 'examples').mkdir()
    # Only buffer accounting differs. Fixtures, lighting and timed loops match.
    harness = harness.replace('* 56', '* 48').replace('+ m.atlas_uvs.capacity() * 8', '')
    (baseline / 'examples/meshing_bench.rs').write_text(harness, encoding='utf-8')
    for label, manifest in [('before', baseline / 'Cargo.toml'), ('after', ROOT / 'Cargo.toml')]:
        command = ['cargo', 'run', '--offline', '--release', '--manifest-path', str(manifest), '-p', 'voxel_core', '--example', 'meshing_bench']
        if label == 'after':
            command.insert(3, '--locked')
        with (args.output / (label + '.csv')).open('w', encoding='utf-8') as output:
            subprocess.run(command, cwd=ROOT, stdout=output, check=True)
print('Baseline:', revision)
print('Results:', args.output)
