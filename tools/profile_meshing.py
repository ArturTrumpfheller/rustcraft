#!/usr/bin/env python3
"""Instrument mesher phases in a disposable core copy; never edits game sources.
Requires the pinned Rust toolchain and cached core dependencies. Timer boundaries
measure wall-clock CPU work, not hardware sampling or GPU/frame performance.
"""
import argparse
import csv
import hashlib
import json
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--revision', required=True, help='Git revision, or WORKTREE for current sources')
parser.add_argument('--output', type=pathlib.Path, required=True)
parser.add_argument('--ao-detail', action='store_true', help='Also isolate packed AO sampling on visible-face inputs; setup excluded.')
args = parser.parse_args()
revision = subprocess.check_output(['git', 'rev-parse', '--verify', ('HEAD' if args.revision == 'WORKTREE' else args.revision) + '^{commit}'], cwd=ROOT, text=True).strip()
args.output.mkdir(parents=True, exist_ok=False)
(args.output / 'revision.txt').write_text(('WORKTREE based on ' if args.revision == 'WORKTREE' else '') + revision + '\n', encoding='utf-8')
with tempfile.TemporaryDirectory(prefix='rustcraft-profile-') as tmp:
    dest = pathlib.Path(tmp)
    names = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', revision, 'crates/voxel_core/src'], cwd=ROOT, text=True).splitlines()
    source_hashes = {}
    for name in names:
        target = dest / name.removeprefix('crates/voxel_core/')
        target.parent.mkdir(parents=True, exist_ok=True)
        data = (ROOT / name).read_bytes() if args.revision == 'WORKTREE' else subprocess.check_output(['git', 'show', revision + ':' + name], cwd=ROOT)
        source_hashes[name] = hashlib.sha256(data).hexdigest()
        target.write_bytes(data)
    (args.output / 'source-sha256.json').write_text(json.dumps(source_hashes, indent=2) + '\n', encoding='utf-8')
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
    if args.ao_detail:
        source += '''
pub fn profile_ao(world:&World, cp:ChunkPos)->(usize,u128) {
    let halo=ao::Occlusion::new(world,cp);
    let mut inputs=Vec::new();
    let Some(chunk)=world.chunk(cp) else {return (0,0)};
    for y in 0..16 {for z in 0..16 {for x in 0..16 {
        let p=[x,y,z]; let lp=LocalPos::new(p).unwrap();
        if !chunk.get(lp).is_cube(){continue;}
        let bp=cp.block_position(lp).unwrap();
        for face in &FACES {
            if !bp.offset(face.normal).is_some_and(|n|world.block(n).is_opaque()) {
                inputs.push((ao::Occlusion::index(p),ao::offsets(face)));
            }
        }
    }}}
    let start=std::time::Instant::now();
    for _ in 0..256 {for &(center,offsets) in &inputs {
        std::hint::black_box(ao::pack(std::hint::black_box(&halo).sample_index(std::hint::black_box(center),std::hint::black_box(offsets))));
    }}
    (inputs.len()*256,start.elapsed().as_nanos())
}
'''
    p.write_text(source, encoding='utf-8')
    p = dest / 'src/lib.rs'
    p.write_text(p.read_text(encoding='utf-8') + '\npub use mesh::profile_take;\n' + ('pub use mesh::profile_ao;\n' if args.ao_detail else ''), encoding='utf-8')
    harness = (ROOT / 'crates/voxel_core/examples/meshing_bench.rs').read_text(encoding='utf-8')
    harness = harness.replace('        samples.sort_unstable();', '''        let p = voxel_core::profile_take();
        eprintln!("PROFILE,{scene},{},{},{},{},{}", p[0], p[1]/p[0], p[2]/p[0], p[3]/p[0], p[4]/p[0]);
        samples.sort_unstable();''', 1)
    if args.ao_detail:
        harness = harness.replace('        samples.sort_unstable();', '''        let mut calls=0; let mut ns=0;
        for &cp in &targets {let (c,n)=voxel_core::profile_ao(&world,cp);calls+=c;ns+=n;}
        eprintln!("AO_DETAIL,{scene},{calls},{ns}");
        samples.sort_unstable();''',1)
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
    if args.ao_detail:
        with (args.output / 'ao-sampling.csv').open('w', encoding='utf-8', newline='') as out:
            writer = csv.writer(out, lineterminator='\n')
            writer.writerow(['scene','calls','elapsed_ns','ns_per_packed_face'])
            for line in result.stderr.splitlines():
                if line.startswith('AO_DETAIL,'):
                    scene,calls,ns=line.split(',')[1:]
                    writer.writerow([scene,calls,ns,float(ns)/int(calls) if int(calls) else 0])
print('Results:', args.output)
