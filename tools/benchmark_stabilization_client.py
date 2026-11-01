#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Run identical headless release ECS measurements against an exact baseline/current.

Only measurement tests/probes are copied into the historical test harness.
No production code or historical algorithms are backported to the baseline.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parent.parent
BASE = 'f9f027daffae8571ce90358685b7e3eff7c2d122'


def replace_once(path, old, new):
    text = path.read_text()
    assert text.count(old) == 1, (path, old, text.count(old))
    path.write_text(text.replace(old, new))


def instrument_client(root):
    # Disposable-source probes only: production retains its bounded maxima.
    # Runtime assertion catches a stale shared-core artifact, independently of
    # Cargo's freshness decision. Only disposable source gains this function.
    core = root / 'crates/voxel_core/src'
    digest = hashlib.sha256(b''.join(
        str(p.relative_to(core)).encode() + b'\0' + p.read_bytes()
        for p in sorted(core.rglob('*.rs')))).hexdigest()
    lib = core / 'lib.rs'
    lib.write_text(lib.read_text() +
                   f'\npub fn slice31_benchmark_fingerprint() -> &\'static str {{ "{digest}" }}\n')
    tests = root / 'client/src/streaming/tests.rs'
    replace_once(tests, 'fn vegetation_streaming_measurements() {',
                 'fn vegetation_streaming_measurements() {\n'
                 f'    assert_eq!(voxel_core::slice31_benchmark_fingerprint(), "{digest}");\n'
                 '    println!("STABILIZATION_SOURCE {}", voxel_core::slice31_benchmark_fingerprint());')
    stream = root / 'client/src/streaming.rs'
    replace_once(stream, '    pub max_install: Duration,',
                 '    pub max_install: Duration,\n    bench_installs: Vec<f64>,\n    bench_loads: Vec<f64>,')
    replace_once(stream, '            max_install: Duration::ZERO,',
                 '            max_install: Duration::ZERO,\n            bench_installs: Vec::new(),\n            bench_loads: Vec::new(),')
    replace_once(stream, '                    state.max_install = state.max_install.max(install_start.elapsed());',
                 '                    let elapsed=install_start.elapsed();\n                    state.bench_installs.push(elapsed.as_secs_f64()*1e6);\n                    state.max_install = state.max_install.max(elapsed);')
    replace_once(stream, 'state.max_load_latency.max(since.elapsed());',
                 'state.max_load_latency.max(since.elapsed());\n                                state.bench_loads.push(since.elapsed().as_secs_f64()*1e6);')
    tests = root / 'client/src/streaming/tests.rs'
    text = tests.read_text()
    start = text.index('fn vegetation_streaming_measurements()')
    point = text.index('        let s = app.world().resource::<StreamState>();',start)
    end = point + len('        let s = app.world().resource::<StreamState>();')
    text = text[:end] + '''
        for (metric, samples) in [("main_install_us", &s.bench_installs), ("queue_install_us", &s.bench_loads)] {
            let mut sorted = samples.clone();
            sorted.sort_by(f64::total_cmp);
            let p = |percent| sorted[(sorted.len()-1)*percent/100];
            println!("STABILIZATION_LATENCY generator={version} metric={metric} count={} p50={:.3} p95={:.3} p99={:.3} raw={samples:?}", sorted.len(),p(50),p(95),p(99));
        }
''' + text[end:]
    tests.write_text(text)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--baseline', default=BASE,
                        help='Exact merged baseline commit (default: Slice29).')
    parser.add_argument('--reverse', action='store_true',
                        help='Measure current before baseline to check order-dependent noise.')
    args = parser.parse_args()
    baseline = subprocess.check_output(['git', 'rev-parse', args.baseline], cwd=ROOT, text=True).strip()
    names = ['slice29', 'slice30'] if baseline == BASE else ['slice30', 'slice31']
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    scratch = ROOT / 'work/stabilization-client'
    order = list(reversed(names)) if args.reverse else names
    binaries_by_name = {}
    provenance = {}
    for name in names:
        root = scratch / name
        if root.exists():
            shutil.rmtree(root)
        root.mkdir(parents=True)
        archive = subprocess.check_output(['git', 'archive', baseline], cwd=ROOT)
        with tarfile.open(fileobj=io.BytesIO(archive)) as t:
            t.extractall(root, filter='data')
        if name == names[1]:
            for directory in ['crates/voxel_core', 'client']:
                shutil.rmtree(root / directory)
                shutil.copytree(ROOT / directory, root / directory)
        provenance[name] = {
            str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for directory in ['crates/voxel_core/src', 'client/src']
            for p in sorted((root / directory).rglob('*.rs'))
        }
        p = root / 'client/src/session/tests.rs'
        added = (ROOT / 'client/src/session/tests.rs').read_text().split(
            '// Slice30 integration and repeatable CPU measurement.')[1]
        if '// Slice30 integration and repeatable CPU measurement.' not in p.read_text():
            p.write_text(p.read_text() + '\n' + added)
        p = root / 'client/src/streaming/tests.rs'
        signature = 'fn vegetation_streaming_measurements()'
        current = (ROOT / 'client/src/streaming/tests.rs').read_text()
        measured = current.split(signature)[1].split('// Slice30 integration')[0]
        p.write_text(p.read_text().split(signature)[0] + signature + measured)
        instrument_client(root)
        # Source copies can carry older mtimes than a shared Cargo fingerprint.
        # Force only the small core package to rebuild; keep Bevy cached. Touch
        # every local Rust input too, so Cargo cannot reuse another copy's core.
        for directory in ['crates/voxel_core', 'client']:
            for source in (root / directory).rglob('*.rs'):
                source.touch()
        with (out / f'{name}-client-build.log').open('w') as log:
            subprocess.run(['cargo', 'clean', '--release', '-p', 'voxel_core',
                            '--target-dir', str(ROOT / 'target')], cwd=root,
                           stdout=log, stderr=log, check=True)
            result = subprocess.run(['cargo', 'test', '--release', '--locked',
                                     '--target-dir', str(ROOT / 'target'), '-p', 'rustcraft',
                                     '--bin', 'rustcraft', '--no-run', '--message-format=json'],
                                    cwd=root, text=True, stdout=subprocess.PIPE,
                                    stderr=log, check=True)
        assert 'Compiling voxel_core ' in (out / f'{name}-client-build.log').read_text(), name
        binaries = [j['executable'] for line in result.stdout.splitlines()
                    if (j := json.loads(line)).get('reason') == 'compiler-artifact'
                    and j.get('executable') and j.get('profile', {}).get('test')]
        assert len(binaries) == 1, binaries
        # Preserve both executables before measuring; shared build output paths
        # may be overwritten when the second source is compiled.
        executable = out / f'{name}-client-bench{Path(binaries[0]).suffix}'
        shutil.copy2(binaries[0], executable)
        binaries_by_name[name] = executable
    for name in order:
        root = scratch / name
        with (out / f'{name}-client.txt').open('w') as log:
            for test in ['vegetation_streaming_measurements',
                         'stabilization_world_switch_measurements']:
                subprocess.run([str(binaries_by_name[name]), test, '--ignored', '--nocapture',
                                '--test-threads=1'], cwd=root, stdout=log,
                               stderr=log, check=True)
        log_path = out / f'{name}-client.txt'
        log_path.write_text(log_path.read_text().rstrip() + '\n')
        print(log_path, flush=True)
    def resources(name):
        records = []
        for line in (out / f'{name}-client.txt').read_text().splitlines():
            if line.startswith(('STABILIZATION_RESIDENCY ', 'VEGETATION_STREAM ')):
                values = dict(token.split('=', 1) for token in line.split()[1:])
                records.append({key: value for key, value in values.items()
                                if key in ['generator', 'descriptors', 'voxel_bytes', 'mesh_assets',
                                           'surfaces', 'cache_columns', 'region_overlays', 'pending',
                                           'fluid_pending', 'retained_mesh_bytes', 'plant_chunk_entities',
                                           'source_cache_bytes']})
            elif line.startswith('STABILIZATION_SWITCH '):
                records.append(line.split('retained_assets=')[1])
        assert len(records) == 5, records
        return records
    assert resources(names[0]) == resources(names[1])
    (out / 'source-sha256.json').write_text(json.dumps(provenance, indent=2) + '\n')
    (out / 'client-method.txt').write_text(
        f'Baseline {baseline}; measurement order {order}. Both executables built first; core cleaned/recompiled for each source; local Rust input mtimes refreshed.\n'
        'Exact baseline/current production sources; identical headless ECS harness. '
        'Visits, worker totals, main-install/queue maxima and session switches are CPU wall times, not GPU measurements.\n')


if __name__ == '__main__':
    main()
