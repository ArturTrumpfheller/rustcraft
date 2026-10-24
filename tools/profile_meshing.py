#!/usr/bin/env python3
"""Instrument mesher phases in a disposable core copy; never edits game sources.
Requires the pinned Rust toolchain and cached core dependencies. Timer boundaries
measure wall-clock CPU work, not hardware sampling or GPU/frame performance.
"""
import argparse
import csv
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--revision', required=True, help='Git revision, or WORKTREE for current sources')
parser.add_argument('--output', type=pathlib.Path, required=True)
args = parser.parse_args()
revision = subprocess.check_output(['git', 'rev-parse', '--verify', ('HEAD' if args.revision == 'WORKTREE' else args.revision) + '^{commit}'], cwd=ROOT, text=True).strip()
args.output.mkdir(parents=True, exist_ok=False)
(args.output / 'revision.txt').write_text(('WORKTREE based on ' if args.revision == 'WORKTREE' else '') + revision + '\n', encoding='utf-8')
with tempfile.TemporaryDirectory(prefix='rustcraft-profile-') as tmp:
    dest = pathlib.Path(tmp)
    names = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', revision, 'crates/voxel_core/src'], cwd=ROOT, text=True).splitlines()
    for name in names:
        target = dest / name.removeprefix('crates/voxel_core/')
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((ROOT / name).read_bytes() if args.revision == 'WORKTREE' else subprocess.check_output(['git', 'show', revision + ':' + name], cwd=ROOT))
    (dest / 'Cargo.toml').write_text('[workspace]\n[package]\nname="voxel_core"\nversion="0.1.0"\nedition="2024"\n[dependencies]\ncrc32fast="=1.5.3"\n', encoding='utf-8')
    p = dest / 'src/mesh.rs'
    source = p.read_text(encoding='utf-8')
    boundaries = [
        ('    let occlusion = ao::Occlusion::new(world, position);', '    let profile_t0 = std::time::Instant::now();\n'),
        ('    let mut active_slices =', '    let profile_t1 = std::time::Instant::now();\n'),
        ('    for y in 0..CHUNK_SIZE {', '    let profile_t2 = std::time::Instant::now();\n'),
        ('\n    for (f, face) in FACES.iter().enumerate() {', '\n    let profile_t3 = std::time::Instant::now();\n'),
    ]
    for marker, timer in boundaries:
        if marker not in source:
            raise RuntimeError('Unsupported mesher: missing instrumentation boundary ' + repr(marker))
        source = source.replace(marker, timer + marker, 1)
    source = source.replace('    mesh\n}', '''    let profile_t4 = std::time::Instant::now();
    PROFILE.with(|cell| {
        let mut p = cell.get(); p[0] += 1;
        for (i, t) in [profile_t1-profile_t0, profile_t2-profile_t1, profile_t3-profile_t2, profile_t4-profile_t3].into_iter().enumerate() { p[i+1] += t.as_nanos(); }
        cell.set(p);
    });
    mesh
}''', 1)
    source += '\nthread_local! { static PROFILE: std::cell::Cell<[u128;5]> = const {std::cell::Cell::new([0;5])}; }\npub fn profile_take() -> [u128;5] { PROFILE.with(|p|p.replace([0;5])) }\n'
    p.write_text(source, encoding='utf-8')
    p = dest / 'src/lib.rs'
    p.write_text(p.read_text(encoding='utf-8') + '\npub use mesh::profile_take;\n', encoding='utf-8')
    harness = (ROOT / 'crates/voxel_core/examples/meshing_bench.rs').read_text(encoding='utf-8')
    harness = harness.replace('        samples.sort_unstable();', '''        let p = voxel_core::profile_take();
        eprintln!("PROFILE,{scene},{},{},{},{},{}", p[0], p[1]/p[0], p[2]/p[0], p[3]/p[0], p[4]/p[0]);
        samples.sort_unstable();''', 1)
    (dest / 'examples').mkdir()
    (dest / 'examples/meshing_bench.rs').write_text(harness, encoding='utf-8')
    with (args.output / 'instrumented.csv').open('w', encoding='utf-8') as out:
        result = subprocess.run(['cargo', 'run', '--offline', '--release', '--manifest-path', str(dest / 'Cargo.toml'), '--example', 'meshing_bench'], cwd=ROOT, stdout=out, stderr=subprocess.PIPE, text=True)
    (args.output / 'build.txt').write_text(result.stderr, encoding='utf-8')
    result.check_returncode()
    with (args.output / 'stages.csv').open('w', encoding='utf-8', newline='') as out:
        writer = csv.writer(out, lineterminator='\n')
        writer.writerow(['scene', 'chunk_calls', 'halo_ns_per_chunk', 'mask_init_ns_per_chunk', 'visibility_ao_ns_per_chunk', 'merge_emit_ns_per_chunk'])
        for line in result.stderr.splitlines():
            if line.startswith('PROFILE,'):
                writer.writerow(line.split(',')[1:])
print('Results:', args.output)
