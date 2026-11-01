#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Compare merged Slice 29 and current v9 with identical disposable instrumentation.

Run without concurrent builds/tests/renderers. Output retains all 32 warm samples
per scene, stage percentiles and exact geometry/resource values for both sources.
No production source is rewritten; all benchmark builds use work/.
"""
import argparse
import csv
import io
import hashlib
import json
import os
import platform
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


def instrument(root):
    core = root / 'crates/voxel_core/src'
    profile = '''
thread_local! {static SLICE30_PROFILE: std::cell::Cell<[u128;6]> = const {std::cell::Cell::new([0;6])};}
pub(crate) fn slice30_add(i:usize,ns:u128){ SLICE30_PROFILE.with(|p|{let mut a=p.get();a[i]+=ns;p.set(a);}); }
pub fn slice30_take()->[u128;6]{SLICE30_PROFILE.with(|p|p.replace([0;6]))}
'''
    p = core / 'worldgen.rs'
    p.write_text(p.read_text() + profile)
    replace_once(core / 'worldgen/local.rs',
                 'super::vegetation::Plan::trees(g, min, max)',
                 '{ let start=std::time::Instant::now(); let v=super::vegetation::Plan::trees(g,min,max); super::slice30_add(0,start.elapsed().as_nanos()); v }')
    replace_once(core / 'worldgen/local.rs', '            vegetation.plants(',
                 '            let plant_start=std::time::Instant::now();\n            vegetation.plants(')
    replace_once(core / 'worldgen/local.rs', '            plan.vegetation = Some(vegetation);',
                 '            super::slice30_add(1,plant_start.elapsed().as_nanos());\n            plan.vegetation = Some(vegetation);')
    replace_once(core / 'streaming.rs', '    let light = crate::lighting::Skylight::for_chunks(source, targets);',
                 '    let light_start=Instant::now();\n    let light = crate::lighting::Skylight::for_chunks(source, targets);\n    crate::worldgen::slice30_add(2,light_start.elapsed().as_nanos());')
    replace_once(core / 'streaming.rs', '            meshes.push((p, crate::mesh::mesh_chunk_lit(source, p, &light)));',
                 '            let mesh_start=Instant::now();\n            meshes.push((p, crate::mesh::mesh_chunk_lit(source, p, &light)));\n            crate::worldgen::slice30_add(3,mesh_start.elapsed().as_nanos());')
    replace_once(core / 'mesh.rs', 'water: crate::water_mesh::mesh_water(world, position, light),',
                 'water: {let start=std::time::Instant::now(); let v=crate::water_mesh::mesh_water(world,position,light); crate::worldgen::slice30_add(4,start.elapsed().as_nanos()); v},')
    replace_once(core / 'mesh.rs', '                    mesh.plants.append(block, p.map(|v| v as f32), bp, light);',
                 '                    let start=std::time::Instant::now();\n                    mesh.plants.append(block, p.map(|v| v as f32), bp, light);\n                    crate::worldgen::slice30_add(5,start.elapsed().as_nanos());')
    bench = (ROOT / 'crates/voxel_core/examples/stabilization_bench.rs').read_text()
    bench = bench.replace('[0u128; 6]; // PROFILE_TAKE', 'voxel_core::worldgen::slice30_take();')
    bench = bench.replace('[0u128; 6]; // STAGE_TAKE', 'voxel_core::worldgen::slice30_take();')
    (root / 'crates/voxel_core/examples/stabilization_bench.rs').write_text(bench)
    shutil.copy2(ROOT / 'crates/voxel_core/examples/stabilization_water_bench.rs',
                 root / 'crates/voxel_core/examples/stabilization_water_bench.rs')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--reverse', action='store_true',
                        help='Measure current before baseline to check order-dependent noise.')
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    scratch = ROOT / 'work/stabilization-benchmark'
    sources = {}
    provenance = {}
    executable_suffix = '.exe' if os.name == 'nt' else ''
    for name in ['slice29', 'slice30']:
        root = scratch / name
        if root.exists():
            shutil.rmtree(root)
        root.mkdir(parents=True)
        archive = subprocess.check_output(['git', 'archive', BASE, 'Cargo.toml', 'Cargo.lock',
                                          'rust-toolchain.toml', 'crates/voxel_core',
                                          'client/Cargo.toml', 'client/src/main.rs'], cwd=ROOT)
        with tarfile.open(fileobj=io.BytesIO(archive)) as t:
            t.extractall(root, filter='data')
        if name == 'slice30':
            shutil.rmtree(root / 'crates/voxel_core')
            shutil.copytree(ROOT / 'crates/voxel_core', root / 'crates/voxel_core')
        provenance[name] = {
            str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted((root / 'crates/voxel_core/src').rglob('*.rs'))
        }
        instrument(root)
        sources[name] = root
    # Build both first so compilation cannot contend with either measurement.
    for name, root in sources.items():
        with (out / f'{name}-build.log').open('w') as log:
            subprocess.run(['cargo', 'build', '--locked', '--release', '--target-dir',
                            str(root / 'target'), '-p', 'voxel_core', '--example',
                            'stabilization_bench', '--example', 'stabilization_water_bench',
                            '--example', 'vegetation_edits_bench'], cwd=root, stdout=log, stderr=log, check=True)
    results = {}
    order = list(reversed(sources)) if args.reverse else list(sources)
    for name in order:
        root = sources[name]
        with (out / f'{name}.csv').open('w') as log:
            subprocess.run([str(root / f'target/release/examples/stabilization_bench{executable_suffix}')],
                           cwd=root, stdout=log, check=True)
        results[name] = list(csv.DictReader((out / f'{name}.csv').open()))
        for example, suffix in [('stabilization_water_bench', 'water'),
                                ('vegetation_edits_bench', 'vegetation-edits')]:
            with (out / f'{name}-{suffix}.csv').open('w') as log:
                subprocess.run([str(root / f'target/release/examples/{example}{executable_suffix}')],
                               cwd=root, stdout=log, check=True)
        print(out / f'{name}.csv', flush=True)
    fields = ['scene', 'seed', 'chunk_x', 'chunk_z', 'sample', 'block_crc',
              'mesh_bytes', 'vertices', 'indices', 'surfaces', 'voxel_bytes', 'cache_bytes']
    assert len(results['slice29']) == len(results['slice30']) == 11 * 32
    for old, new in zip(results['slice29'], results['slice30']):
        assert all(old[f] == new[f] for f in fields), (old, new)
    old_water = list(csv.DictReader((out / 'slice29-water.csv').open()))
    new_water = list(csv.DictReader((out / 'slice30-water.csv').open()))
    assert len(old_water) == len(new_water) == 24
    for old, new in zip(old_water, new_water):
        assert all(old[f] == new[f] for f in ['processed', 'pending', 'edits', 'drops'])
    with (out / 'percentiles.csv').open('w', newline='') as file:
        writer = csv.writer(file, lineterminator="\n")
        writer.writerow(['build', 'scene', 'metric', 'samples', 'p50', 'p95', 'p99'])
        for name, rows in results.items():
            for scene in dict.fromkeys(row['scene'] for row in rows):
                group = [r for r in rows if r['scene'] == scene]
                for metric in rows[0]:
                    if metric.endswith('_us') or metric in ['peak_heap', 'mesh_bytes', 'voxel_bytes', 'cache_bytes']:
                        values = sorted(float(row[metric]) for row in group)
                        writer.writerow([name, scene, metric, len(values)] +
                                        [f'{values[(len(values)-1)*p//100]:.3f}' for p in [50, 95, 99]])
    (out / 'source-sha256.json').write_text(json.dumps(provenance, indent=2) + '\n')
    (out / 'method.txt').write_text(
        f'Baseline {BASE}\nCurrent working tree based on ' + subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True) +
        subprocess.check_output(['rustc', '--version'], text=True) +
        platform.platform() + '\n' +
        '11 identical full-height generator9 column jobs; 4 warmups + 32 samples each. '
        'Same source instrumentation and requested-heap allocator. Core install is CPU descriptor publication, '
        'not Bevy installation or GPU upload. Worker us is wall time; worker_cpu_us is thread CPU on Linux. '
        'Opaque us is combined mesher minus measured water and per-plant append calls; loop/clock overhead stays in opaque. '
        'Percentile index floor((N-1)*p/100). Authoritative CRC and all geometry counts/bytes match exactly. '
        f'Measurement order: {order}. '
        'Cloud CPU measurements; native GPU, frame times and driver allocations unavailable.\n')


if __name__ == '__main__':
    main()
