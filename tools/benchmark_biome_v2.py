#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Exact merged Slice36 v11/current v12 CPU/requested heap comparison.
Both source exports build first. Run measurements without concurrent workloads.
"""
import argparse,csv,hashlib,io,json,os,platform,shutil,subprocess,tarfile
from pathlib import Path
from benchmark_stabilization import instrument,ROOT
BASE='3d0f13c7e603ed060d53a722cf73c51619ade308'
def wrap(path,signature,index):
 s=path.read_text();start=s.index('{',s.index(signature));level=1;end=start+1
 while level:
  level+=(s[end]=='{')-(s[end]=='}');end+=1
 body=s[start+1:end-1]
 s=s[:start+1]+f'\nlet stage_start=std::time::Instant::now();let result=(||{{{body}}})();super::slice30_add({index},stage_start.elapsed().as_nanos());result\n'+s[end-1:];path.write_text(s)
def prepare(root,current):
 instrument(root)
 p=root/'crates/voxel_core/src/worldgen.rs';p.write_text(p.read_text().replace('[u128;16]','[u128;20]').replace('[0;16]','[0;20]'))
 local=root/'crates/voxel_core/src/worldgen/local.rs';s=local.read_text()
 s=s.replace('super::terrain_v4::columns(g.config.seed, min)','{ let start=std::time::Instant::now();let v=super::terrain_v4::columns(g.config.seed,min);super::slice30_add(15,start.elapsed().as_nanos());v }')
 if current:
  s=s.replace('super::biome_v2::columns(g.config.seed, min)','{ let start=std::time::Instant::now();let v=super::biome_v2::columns(g.config.seed,min);super::slice30_add(15,start.elapsed().as_nanos());v }')
  v=root/'crates/voxel_core/src/worldgen/vegetation.rs';t=v.read_text().replace('Self::trees_with_surface(g, min, max, Some(columns), Some(ecology))','{let start=std::time::Instant::now();let v=Self::trees_with_surface(g,min,max,Some(columns),Some(ecology));super::slice30_add(0,start.elapsed().as_nanos());v}');v.write_text(t)
  wrap(root/'crates/voxel_core/src/worldgen/biome_v2.rs','fn fields(',16)
  wrap(root/'crates/voxel_core/src/worldgen/biome_v2.rs','fn evaluate_with_coast(',17)
 else:
  wrap(root/'crates/voxel_core/src/worldgen/biome.rs','fn classify(',17)
 wrap(root/'crates/voxel_core/src/worldgen/biome.rs','fn sample_climate(',16)
 local.write_text(s)
 bench=root/'crates/voxel_core/examples/stabilization_bench.rs';s=bench.read_text().replace('raw_requested_bytes"','raw_requested_bytes,climate_us,classification_us"').replace('{raw_requested}",','{raw_requested},{:.3},{:.3}",').replace('planning[15] as f64 / 1000.','planning[15] as f64 / 1000., planning[16] as f64 / 1000., planning[17] as f64 / 1000.')
 bench.write_text(s)
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--samples',type=int,default=128);p.add_argument('--warmups',type=int,default=8);p.add_argument('--build-only',action='store_true');p.add_argument('--reuse',action='store_true');p.add_argument('--reverse',action='store_true');a=p.parse_args();assert 1<=a.samples<=10000 and 0<=a.warmups<=1000
 out=a.output.resolve();out.mkdir(parents=True,exist_ok=True);scratch=ROOT/'work/biome-v2-benchmark';names=['slice36','slice37']
 if not a.reuse:
  sources={}
  for name in names:
   root=scratch/name
   if root.exists():shutil.rmtree(root)
   root.mkdir(parents=True)
   data=subprocess.check_output(['git','archive',BASE,'Cargo.toml','Cargo.lock','rust-toolchain.toml','crates/voxel_core','client/Cargo.toml','client/src/main.rs'],cwd=ROOT)
   with tarfile.open(fileobj=io.BytesIO(data)) as t:t.extractall(root,filter='data')
   if name=='slice37':shutil.rmtree(root/'crates/voxel_core');shutil.copytree(ROOT/'crates/voxel_core',root/'crates/voxel_core')
   sources[name]={str(f.relative_to(root)):hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted((root/'crates/voxel_core/src').rglob('*.rs'))}
   prepare(root,name=='slice37')
   with (out/f'{name}-build.log').open('w') as log:subprocess.run(['cargo','build','--release','--locked','--target-dir',str(root/'target'),'-p','voxel_core','--example','stabilization_bench','--example','vegetation_edits_bench'],cwd=root,stdout=log,stderr=log,check=True)
   for example,label in [('stabilization_bench','bench'),('vegetation_edits_bench','lifecycle')]:shutil.copy2(root/f'target/release/examples/{example}',out/f'{name}-{label}')
  (out/'sources.json').write_text(json.dumps(dict(baseline=BASE,files=sources,binary_sha256={n+'-'+b:hashlib.sha256((out/(n+'-'+b)).read_bytes()).hexdigest() for n in names for b in ['bench','lifecycle']},rustc=subprocess.check_output(['rustc','--version'],text=True),platform=platform.platform(),harness_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()),indent=2)+'\n')
 if a.build_only:return
 manifest=json.loads((out/'sources.json').read_text());order=list(reversed(names)) if a.reverse else names
 for name in order:
  version=11 if name=='slice36' else 12
  for kind in ['bench','lifecycle']:
   assert hashlib.sha256((out/f'{name}-{kind}').read_bytes()).hexdigest()==manifest['binary_sha256'][name+'-'+kind]
   with (out/f'{name}{"-lifecycle" if kind=="lifecycle" else ""}.csv').open('w') as log:
    subprocess.run([str(out/f'{name}-{kind}')],cwd=scratch/name,stdout=log,check=True,env={**os.environ,'RUSTCRAFT_BENCH_GENERATOR':str(version),'RUSTCRAFT_BENCH_BIOME_V2':'1','RUSTCRAFT_BENCH_RAW_LIFECYCLE':'1','RUSTCRAFT_BENCH_SAMPLES':str(a.samples),'RUSTCRAFT_BENCH_WARMUPS':str(a.warmups)})
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
 (out/'method.json').write_text(json.dumps(dict(samples=a.samples,warmups=a.warmups,order=order,percentile='floor((n-1)*p/100)',limitations='Clock instrumentation overhead included; requested heap excludes allocator metadata/stacks/GPU. Climate_us includes complete old climate queries (also inherited water admission in v12) plus v12 new climate fields; shared terrain fields are excluded. Classification includes v12 material/coast checks. The same geometry with different ecosystems has intentionally different mesh/light payloads.'),indent=2)+'\n')
if __name__=='__main__':main()
