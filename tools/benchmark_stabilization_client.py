#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Run identical headless release ECS measurements against exact Slice29/current.

Only the new session measurement is copied into the historical test harness.
No production code or historical algorithms are backported to the baseline.
"""
import argparse
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parent.parent
BASE = 'f9f027daffae8571ce90358685b7e3eff7c2d122'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--reverse', action='store_true',
                        help='Measure current before baseline to check order-dependent noise.')
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    scratch = ROOT / 'work/stabilization-client'
    order = ['slice30', 'slice29'] if args.reverse else ['slice29', 'slice30']
    for name in order:
        root = scratch / name
        if root.exists():
            shutil.rmtree(root)
        root.mkdir(parents=True)
        archive = subprocess.check_output(['git', 'archive', BASE], cwd=ROOT)
        with tarfile.open(fileobj=io.BytesIO(archive)) as t:
            t.extractall(root, filter='data')
        # One production difference in this slice: diagnostic height accounting.
        # Copy current core only for the resulting build, preserving baseline.
        if name == 'slice30':
            shutil.copy2(ROOT / 'crates/voxel_core/src/streaming.rs',
                         root / 'crates/voxel_core/src/streaming.rs')
        p = root / 'client/src/session/tests.rs'
        added = (ROOT / 'client/src/session/tests.rs').read_text().split(
            '// Slice30 integration and repeatable CPU measurement.')[1]
        p.write_text(p.read_text() + '\n' + added)
        p = root / 'client/src/streaming/tests.rs'
        signature = 'fn vegetation_streaming_measurements()'
        current = (ROOT / 'client/src/streaming/tests.rs').read_text()
        measured = current.split(signature)[1].split('// Slice30 integration')[0]
        p.write_text(p.read_text().split(signature)[0] + signature + measured)
        # Shared Bevy build cache; measurement starts only after compilation.
        with (out / f'{name}-client-build.log').open('w') as log:
            result = subprocess.run(['cargo', 'test', '--release', '--locked',
                                     '--target-dir', str(ROOT / 'target'), '-p', 'rustcraft',
                                     '--bin', 'rustcraft', '--no-run', '--message-format=json'],
                                    cwd=root, text=True, stdout=subprocess.PIPE,
                                    stderr=log, check=True)
        binaries = [j['executable'] for line in result.stdout.splitlines()
                    if (j := json.loads(line)).get('reason') == 'compiler-artifact'
                    and j.get('executable') and j.get('profile', {}).get('test')]
        assert len(binaries) == 1, binaries
        with (out / f'{name}-client.txt').open('w') as log:
            for test in ['vegetation_streaming_measurements',
                         'stabilization_world_switch_measurements']:
                subprocess.run([binaries[0], test, '--ignored', '--nocapture',
                                '--test-threads=1'], cwd=root, stdout=log,
                               stderr=log, check=True)
        log_path = out / f'{name}-client.txt'
        log_path.write_text(log_path.read_text().rstrip() + '\n')
        print(log_path, flush=True)


if __name__ == '__main__':
    main()
