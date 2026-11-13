#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Matched exact Slice38/current caves CPU/requested heap, both orders.
Exports baseline, copies current core, adds clocks only in disposable sources.
"""
import argparse,csv,hashlib,io,json,os,platform,shutil,subprocess,tarfile
from pathlib import Path
from benchmark_stabilization import instrument,ROOT
from benchmark_terrain_balance import wrap
BASE='c6d36393c037e4295bf93c6a4efbc9fa12dd053c'
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--samples',type=int,default=128);p.add_argument('--warmups',type=int,default=8);p.add_argument('--reverse',action='store_true');p.add_argument('--reuse',action='store_true');p.add_argument('--build-only',action='store_true');a=p.parse_args();assert 1<=a.samples<=10000
 out=a.output.resolve();out.mkdir(parents=True,exist_ok=True);scratch=ROOT/'work/cave-v3-benchmark';names=['slice38','slice39']
 if not a.reuse:
  sources={}
  for name in names:
   root=scratch/name
   if root.exists():shutil.rmtree(root)
   root.mkdir(parents=True)
   data=subprocess.check_output(['git','archive',BASE,'Cargo.toml','Cargo.lock','rust-toolchain.toml','crates/voxel_core','client/Cargo.toml','client/src/main.rs'],cwd=ROOT)
   with tarfile.open(fileobj=io.BytesIO(data)) as t:t.extractall(root,filter='data')
   if name=='slice39':shutil.rmtree(root/'crates/voxel_core');shutil.copytree(ROOT/'crates/voxel_core',root/'crates/voxel_core')
   sources[name]={str(f.relative_to(root)):hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted((root/'crates/voxel_core/src').rglob('*.rs'))}
   instrument(root)
   w=root/'crates/voxel_core/src/worldgen.rs';w.write_text(w.read_text().replace('[u128;16]','[u128;20]').replace('[0;16]','[0;20]'))
   if name=='slice38':
    wrap(root/'crates/voxel_core/src/worldgen/caves.rs','fn new(seed:',15)
    wrap(root/'crates/voxel_core/src/worldgen/caves.rs','fn carve_if(',16)
   else:
    wrap(root/'crates/voxel_core/src/worldgen/caves_v3.rs','fn new(g:',15)
    # New dispatcher includes the entire v14 graph and room-field plan.
    wrap(root/'crates/voxel_core/src/worldgen/caves_v3.rs','fn new(seed:',17)
    wrap(root/'crates/voxel_core/src/worldgen/caves_v3.rs','fn carve_if(',16)
   b=root/'crates/voxel_core/examples/stabilization_bench.rs';s=b.read_text()
   s=s.replace('raw_requested_bytes"','raw_requested_bytes,cave_plan_us,cave_carve_us"').replace('{raw_requested}",','{raw_requested},{:.3},{:.3}",').replace('planning[15] as f64 / 1000.','planning[17] as f64 / 1000.,planning[15] as f64 / 1000.,planning[16] as f64 / 1000.').replace('density_plan_us','room_field_us')
   marker='    for (name, seed, c) in scenes {'
   s=s.replace(marker,"    scenes = vec![(\"quiet-plain\",0,[-220,-252]),(\"cavern\",42,[-13,5]),(\"intersection\",42,[-12,5]),(\"narrow\",0,[-7,12]),(\"deep\",42,[-39,-38]),(\"mountain\",42,[-45,-36]),(\"entrance\",2026,[5,-12]),(\"distant\",2026,[-1464,1272])];\n"+marker)
   b.write_text(s)
   with (out/f'{name}-build.log').open('w') as log:subprocess.run(['cargo','build','--release','--locked','--target-dir',str(root/'target'),'-p','voxel_core','--example','stabilization_bench','--example','vegetation_edits_bench'],cwd=root,stdout=log,stderr=log,check=True)
   for example,label in [('stabilization_bench','bench'),('vegetation_edits_bench','lifecycle')]:shutil.copy2(root/f'target/release/examples/{example}',out/f'{name}-{label}')
  (out/'sources.json').write_text(json.dumps(dict(baseline=BASE,files=sources,binary_sha256={n+'-'+b:hashlib.sha256((out/(n+'-'+b)).read_bytes()).hexdigest() for n in names for b in ['bench','lifecycle']},rustc=subprocess.check_output(['rustc','--version'],text=True),platform=platform.platform(),harness_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()),indent=2)+'\n')
 if a.build_only:return
 manifest=json.loads((out/'sources.json').read_text());order=names[::-1] if a.reverse else names
 for name in order:
  for kind in ['bench','lifecycle']:
   assert hashlib.sha256((out/f'{name}-{kind}').read_bytes()).hexdigest()==manifest['binary_sha256'][name+'-'+kind]
   with (out/f'{name}{"-lifecycle" if kind=="lifecycle" else ""}.csv').open('w') as log:
    subprocess.run([str(out/f'{name}-{kind}')],cwd=scratch/name,stdout=log,check=True,env={**os.environ,'RUSTCRAFT_BENCH_GENERATOR':'13' if name=='slice38' else '14','RUSTCRAFT_BENCH_BIOME_V2':'1','RUSTCRAFT_BENCH_RAW_LIFECYCLE':'1','RUSTCRAFT_BENCH_SAMPLES':str(a.samples),'RUSTCRAFT_BENCH_WARMUPS':str(a.warmups)})
 with (out/'percentiles.csv').open('w',newline='') as f:
  w=csv.writer(f,lineterminator='\n');w.writerow(['build','scene','metric','n','p50','p95','p99','max'])
  for name in names:
   for suffix in ['', '-lifecycle']:
    rows=list(csv.DictReader((out/f'{name}{suffix}.csv').open()))
    if suffix:
     for r in rows:r['scene']='lifecycle'
    for scene in dict.fromkeys(r['scene'] for r in rows):
     group=[r for r in rows if r['scene']==scene]
     for metric in group[0]:
      if metric.endswith(('_us','_bytes')) or metric in ['peak_heap','allocation_calls','requested_bytes','raw_peak_heap','raw_allocation_calls','surfaces','vertices','light_cells']:
       v=sorted(float(r[metric]) for r in group);w.writerow([name,scene,metric,len(v)]+[v[(len(v)-1)*q//100] for q in [50,95,99,100]])
 (out/'method.json').write_text(json.dumps(dict(samples=a.samples,warmups=a.warmups,order=order,percentile='floor((n-1)*p/100)',limitations='Clock overhead included. Requested heap excludes allocator metadata/stacks/GPU. Mesh/light output intentionally differs. No native FPS/GPU claims.'),indent=2)+'\n')
if __name__=='__main__':main()
