#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Exact Slice34 v9 / current v10 full-height CPU and requested-heap comparison.
All raw observations and adverse maxima retained. Run with no concurrent workloads.
"""
import argparse,csv,hashlib,io,json,os,platform,shutil,subprocess,tarfile
from pathlib import Path
from benchmark_stabilization import instrument,ROOT
BASE='aab49019b5d4f2ceccef1b748c458df46375cc4b'
def run(cmd,cwd,log,env=None):
    subprocess.run(cmd,cwd=cwd,stdout=log,stderr=subprocess.STDOUT,env=env,check=True)
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--samples',type=int,default=128);p.add_argument('--warmups',type=int,default=8);p.add_argument('--build-only',action='store_true');p.add_argument('--reuse',action='store_true');p.add_argument('--reverse',action='store_true');p.add_argument('--adverse',action='store_true',help='Use the photographed steep mountain flank instead of the nine ordinary scenes');a=p.parse_args()
    assert 1<=a.samples<=10000 and 0<=a.warmups<=1000
    out=a.output.resolve();out.mkdir(parents=True,exist_ok=True);scratch=ROOT/'work/terrain-v3-benchmark';sources={}
    if not a.reuse:
        for name in ['slice34','slice35']:
            root=scratch/name
            if root.exists():shutil.rmtree(root)
            root.mkdir(parents=True)
            data=subprocess.check_output(['git','archive',BASE,'Cargo.toml','Cargo.lock','rust-toolchain.toml','crates/voxel_core','client/Cargo.toml','client/src/main.rs'],cwd=ROOT)
            with tarfile.open(fileobj=io.BytesIO(data)) as t:t.extractall(root,filter='data')
            if name=='slice35':
                shutil.rmtree(root/'crates/voxel_core');shutil.copytree(ROOT/'crates/voxel_core',root/'crates/voxel_core')
            sources[name]={str(f.relative_to(root)):hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted((root/'crates/voxel_core/src').rglob('*.rs'))}
            if name=='slice34':
                f=root/'crates/voxel_core/src/worldgen.rs'
                f.write_text(f.read_text()+'\nimpl Generator { pub fn terrain_admission_cache_entries(&self)->usize { 0 } }\n')
            instrument(root)
            shutil.copy2(ROOT/'crates/voxel_core/examples/vegetation_edits_bench.rs',root/'crates/voxel_core/examples/vegetation_edits_bench.rs')
            with (out/f'{name}-build.log').open('w') as log:run(['cargo','build','--release','--locked','--target-dir',str(root/'target'),'-p','voxel_core','--example','stabilization_bench','--example','vegetation_edits_bench'],root,log)
            binary=root/'target/release/examples/stabilization_bench';shutil.copy2(binary,out/f'{name}-bench')
            shutil.copy2(root/'target/release/examples/vegetation_edits_bench',out/f'{name}-lifecycle')
        (out/'sources.json').write_text(json.dumps({'baseline':BASE,'files':sources,'binary_sha256':{name:hashlib.sha256((out/(name if name.endswith('-lifecycle') else name+'-bench')).read_bytes()).hexdigest() for name in [*sources,*[n+'-lifecycle' for n in sources]]},'rustc':subprocess.check_output(['rustc','--version'],text=True),'platform':platform.platform(),'harness_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},indent=2)+'\n')
    if a.build_only:return
    manifest=json.loads((out/'sources.json').read_text())
    order=[('slice34',9),('slice35',10)]
    if a.reverse:order.reverse()
    for name,version in order:
        assert hashlib.sha256((out/f'{name}-bench').read_bytes()).hexdigest()==manifest['binary_sha256'][name]
        with (out/f'{name}.csv').open('w') as log:run([str(out/f'{name}-bench')],scratch/name,log,{**os.environ,'RUSTCRAFT_BENCH_GENERATOR':str(version),'RUSTCRAFT_BENCH_TERRAIN_V3':'1',**({'RUSTCRAFT_BENCH_ADVERSE_TERRAIN':'1'} if a.adverse else {}),'RUSTCRAFT_BENCH_SAMPLES':str(a.samples),'RUSTCRAFT_BENCH_WARMUPS':str(a.warmups)})
    for name,version in order:
        assert hashlib.sha256((out/f'{name}-lifecycle').read_bytes()).hexdigest()==manifest['binary_sha256'][name+'-lifecycle']
        with (out/f'{name}-lifecycle.csv').open('w') as log:
            run([str(out/f'{name}-lifecycle')],scratch/name,log,{**os.environ,'RUSTCRAFT_BENCH_GENERATOR':str(version),'RUSTCRAFT_BENCH_RAW_LIFECYCLE':'1'})
    # Separate current-v9 compatibility run uses the same matched locations.
    with (out/'slice35-v9.csv').open('w') as log:run([str(out/'slice35-bench')],scratch/'slice35',log,{**os.environ,'RUSTCRAFT_BENCH_GENERATOR':'9','RUSTCRAFT_BENCH_TERRAIN_V3':'1',**({'RUSTCRAFT_BENCH_ADVERSE_TERRAIN':'1'} if a.adverse else {}),'RUSTCRAFT_BENCH_SAMPLES':str(a.samples),'RUSTCRAFT_BENCH_WARMUPS':str(a.warmups)})
    names=['slice34','slice35','slice35-v9'];rows={n:list(csv.DictReader((out/f'{n}.csv').open())) for n in names}
    for old,new in zip(rows['slice34'],rows['slice35-v9']):
        for key in ['scene','seed','block_crc','geometry_crc','light_crc','vertices','indices','surfaces','mesh_bytes','voxel_bytes','cache_bytes']:assert old[key]==new[key],(key,old,new)
    with (out/'percentiles.csv').open('w',newline='') as f:
        w=csv.writer(f,lineterminator="\n");w.writerow(['build','scene','metric','n','p50','p95','p99','max'])
        for name in names:
            for scene in dict.fromkeys(r['scene'] for r in rows[name]):
                group=[r for r in rows[name] if r['scene']==scene]
                for metric in group[0]:
                    if metric.endswith('_us') or metric.endswith('_bytes') or metric in ['peak_heap','allocation_calls','requested_bytes','surfaces','vertices','light_cells','compact_chunks','air_chunks','stone_chunks','admission_cache_entries']:
                        values=sorted(float(r[metric]) for r in group)
                        w.writerow([name,scene,metric,len(values)]+[values[(len(values)-1)*q//100] for q in [50,95,99,100]])
    with (out/'lifecycle-percentiles.csv').open('w',newline='') as f:
        w=csv.writer(f,lineterminator="\n");w.writerow(['build','metric','n','p50','p95','p99','max'])
        for name,_ in order:
            data=list(csv.DictReader((out/f'{name}-lifecycle.csv').open()))
            assert len(data)==24 and all(r['plants_removed']=='8' for r in data)
            for metric in ['break_us','save_us','reload_us','regeneration_us']:
                v=sorted(float(r[metric]) for r in data);w.writerow([name,metric,len(v)]+[v[(len(v)-1)*q//100] for q in [50,95,99,100]])
    (out/'method.json').write_text(json.dumps({'samples':a.samples,'warmups':a.warmups,'adverse':a.adverse,'order':order,'percentile':'floor((n-1)*p/100)','compatibility':'All matched v9 block/light/geometry signatures and resource counts equal','limitations':'Requested heap excludes allocator metadata, stacks, GPU; CPU column installation is not Bevy/GPU installation.'},indent=2)+'\n')
if __name__=='__main__':main()
