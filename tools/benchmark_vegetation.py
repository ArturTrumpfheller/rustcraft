#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Matched Slice28/current CPU benchmark in disposable, instrumented source copies.

No production source is rewritten. Timers wrap real tree/plant planning, lighting
and meshing. Heap meter counts requested heap bytes, excluding stacks/allocator
metadata/GPU. Run from the repository with Python3 and the pinned Rust toolchain.
"""
import argparse
import io
from pathlib import Path
import shutil
import subprocess
import tarfile

ROOT=Path(__file__).resolve().parent.parent
BASE='a3acd940f9fb0a4ca4bcabde555077097ac0ff6e'

def instrument(root, baseline):
    core=root/'crates/voxel_core/src'
    p=core/'worldgen.rs'
    p.write_text(p.read_text()+'''
thread_local! {static VEGETATION_PROFILE: std::cell::Cell<[u128;4]> = const {std::cell::Cell::new([0;4])};}
pub(crate) fn profile_add(i:usize,ns:u128){ VEGETATION_PROFILE.with(|p|{let mut a=p.get();a[i]+=ns;p.set(a);}); }
pub fn vegetation_profile_take()->[u128;4]{VEGETATION_PROFILE.with(|p|p.replace([0;4]))}
''')
    p=core/'worldgen/local.rs';s=p.read_text()
    s=s.replace('super::vegetation::Plan::trees(g, min, max)', '{ let start=std::time::Instant::now(); let value=super::vegetation::Plan::trees(g,min,max); super::profile_add(0,start.elapsed().as_nanos()); value }')
    s=s.replace('trees(g, min, max)', '{ let start=std::time::Instant::now(); let value=trees(g,min,max); super::profile_add(0,start.elapsed().as_nanos()); value }')
    s=s.replace('            vegetation.plants(', '            let plant_start=std::time::Instant::now();\n            vegetation.plants(')
    s=s.replace('            plan.vegetation = Some(vegetation);','            super::profile_add(1,plant_start.elapsed().as_nanos());\n            plan.vegetation = Some(vegetation);')
    p.write_text(s)
    p=core/'streaming.rs';s=p.read_text().replace('    let light = crate::lighting::Skylight::for_chunks(source, targets);','    let light_start=Instant::now();\n    let light = crate::lighting::Skylight::for_chunks(source, targets);\n    crate::worldgen::profile_add(2,light_start.elapsed().as_nanos());')
    s=s.replace('            meshes.push((p, crate::mesh::mesh_chunk_lit(source, p, &light)));','            let mesh_start=Instant::now();\n            meshes.push((p, crate::mesh::mesh_chunk_lit(source, p, &light)));\n            crate::worldgen::profile_add(3,mesh_start.elapsed().as_nanos());')
    p.write_text(s)
    bench=(ROOT/'crates/voxel_core/examples/vegetation_bench.rs').read_text()
    bench=bench.replace('let planning = [0u128; 2]; // PROFILE_TAKE','let planning = voxel_core::worldgen::vegetation_profile_take();').replace('let planning=[0u128;2]; // PROFILE_TAKE','let planning=voxel_core::worldgen::vegetation_profile_take();')
    bench=bench.replace('let stage = [0u128; 2]; // STAGE_TAKE','let stage = voxel_core::worldgen::vegetation_profile_take();').replace('let stage=[0u128;2]; // STAGE_TAKE','let stage=voxel_core::worldgen::vegetation_profile_take();')
    bench=bench.replace('stage[0] as f64','stage[2] as f64').replace('stage[1] as f64','stage[3] as f64')
    if baseline:
        bench=bench.replace('s.vegetation_bytes()','0usize')
        bench=bench.replace(' + m.plants.positions.len()*56 + m.plants.indices.len()*4','').replace(' + m.plants.positions.len() * 56 + m.plants.indices.len() * 4','')
        import re
        bench=re.sub(r'\s*\+\s*m\.plants\.(positions|indices)\.len\(\)(?:\s*\*\s*(?:56|4))?', '',bench)
        bench=re.sub(r'\s*\+\s*usize::from\(!m\.plants\.indices\.is_empty\(\)\)', '',bench)
    (root/'crates/voxel_core/examples/vegetation_bench.rs').write_text(bench)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
    scratch=ROOT/'work/vegetation-benchmark';scratch.mkdir(parents=True,exist_ok=True)
    for name,version in [('slice28',8),('slice29',9)]:
        root=scratch/name
        if root.exists():shutil.rmtree(root)
        root.mkdir()
        if name=='slice28':
            archive=subprocess.check_output(['git','archive',BASE,'Cargo.toml','Cargo.lock','rust-toolchain.toml','crates/voxel_core','client/Cargo.toml','client/src/main.rs'],cwd=ROOT)
            with tarfile.open(fileobj=io.BytesIO(archive)) as t:t.extractall(root,filter='data')
        else:
            for f in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','client/Cargo.toml','client/src/main.rs']:
                dest=root/f;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/f,dest)
            shutil.copytree(ROOT/'crates/voxel_core',root/'crates/voxel_core')
        instrument(root,name=='slice28')
        with (out/f'{name}.csv').open('w') as stdout,(out/f'{name}-build.log').open('w') as stderr:
            subprocess.run(['cargo','run','--locked','--release','--target-dir',str(scratch/'target'),'-p','voxel_core','--example','vegetation_bench','--',str(version)],cwd=root,stdout=stdout,stderr=stderr,check=True)
        print(out/f'{name}.csv',flush=True)
    (out/'method.txt').write_text(f'Baseline {BASE}\n'+subprocess.check_output(['rustc','--version'],text=True)+subprocess.check_output(['uname','-srm'],text=True)+'48 raw columns, 24 warm worker samples after 4 warmups; cold work excluded. Each raw column includes its bounded planning and all 16 vertical descriptors. Same heap meter and stage instrumentation. Core install timings exclude Bevy GPU installation; no hardware GPU claim. This cloud container has a two-CPU cgroup quota.\n')
if __name__=='__main__':main()
