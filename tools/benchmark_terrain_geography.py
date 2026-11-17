#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Instrumented full-column v16/v17 CPU, mesh and requested heap comparison.

Both arms execute the same source/executable, with frozen historical v16 dispatch.
All instrumentation is confined to an isolated copied workspace. Run after
builds, surveys and renderers have finished; warm samples and outliers retained.
"""
import argparse
import csv
import hashlib
import json
import os
import platform
import shutil
import subprocess
from pathlib import Path
from benchmark_stabilization import instrument, ROOT
from benchmark_terrain_balance import wrap


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--samples',type=int,default=32)
    p.add_argument('--warmups',type=int,default=4)
    p.add_argument('--build-only',action='store_true')
    p.add_argument('--reuse',action='store_true')
    a=p.parse_args();out=a.output.resolve();out.mkdir(parents=True,exist_ok=True)
    root=ROOT/'work/slice43/core-benchmark'
    if not a.reuse:
        root.mkdir(parents=True,exist_ok=False)
        for f in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','client/Cargo.toml','client/src/main.rs']:
            target=root/f;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/f,target)
        shutil.copytree(ROOT/'crates/voxel_core',root/'crates/voxel_core')
        hashes={str(f.relative_to(ROOT)):hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted((ROOT/'crates/voxel_core/src').rglob('*.rs'))}
        instrument(root)
        path=root/'crates/voxel_core/src/worldgen.rs';s=path.read_text().replace('[u128;16]','[u128;20]').replace('[0;16]','[0;20]');path.write_text(s)
        wrap(root/'crates/voxel_core/src/worldgen/terrain_v4.rs','fn profiles_with(',18)
        path=root/'crates/voxel_core/examples/stabilization_bench.rs'
        s=path.read_text().replace('raw_requested_bytes"','raw_requested_bytes,terrain_profiles_us"').replace('{raw_requested}",','{raw_requested},{:.3}",').replace('planning[15] as f64 / 1000.','planning[15] as f64 / 1000.,planning[18] as f64 / 1000.')
        path.write_text(s)
        with (out/'build.log').open('w') as f:
            subprocess.run(['cargo','build','--release','--locked','-p','voxel_core','--example','stabilization_bench'],cwd=root,stdout=f,stderr=f,check=True)
        binary=root/'target/release/examples/stabilization_bench'
        manifest=dict(base=subprocess.check_output(['git','rev-parse','origin/main'],cwd=ROOT,text=True).strip(),files=hashes,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),harness_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),instrumented_files={str(f.relative_to(root)):hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted((root/'crates/voxel_core/src').rglob('*.rs'))},platform=platform.platform(),rustc=subprocess.check_output(['rustc','--version'],text=True).strip())
        (out/'sources.json').write_text(json.dumps(manifest,indent=2)+'\n')
    if a.build_only:return
    manifest=json.loads((out/'sources.json').read_text())
    for name,h in manifest['files'].items():assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==h,name
    binary=root/'target/release/examples/stabilization_bench'
    assert hashlib.sha256(binary.read_bytes()).hexdigest()==manifest['binary_sha256']
    for order,versions in [('forward',[16,17]),('reverse',[17,16])]:
        for v in versions:
            with (out/f'{order}-v{v}.csv').open('w') as f:
                subprocess.run([str(binary)],cwd=root,stdout=f,check=True,env={**os.environ,'RUSTCRAFT_BENCH_GENERATOR':str(v),'RUSTCRAFT_BENCH_BIOME_V2':'1','RUSTCRAFT_BENCH_SAMPLES':str(a.samples),'RUSTCRAFT_BENCH_WARMUPS':str(a.warmups)})
    with (out/'percentiles.csv').open('w',newline='') as f:
        writer=csv.writer(f,lineterminator='\n');writer.writerow(['order','version','scene','metric','n','p50','p95','p99','max'])
        for order in ['forward','reverse']:
            for v in [16,17]:
                rows=list(csv.DictReader((out/f'{order}-v{v}.csv').open()))
                for scene in dict.fromkeys(r['scene'] for r in rows):
                    group=[r for r in rows if r['scene']==scene]
                    for metric in group[0]:
                        if metric.endswith(('_us','_bytes')) or metric in ['peak_heap','raw_peak_heap','vertices','indices','surfaces']:
                            values=sorted(float(r[metric]) for r in group)
                            writer.writerow([order,v,scene,metric,len(values)]+[values[(len(values)-1)*q//100] for q in [50,95,99,100]])
    (out/'method.json').write_text(json.dumps(dict(samples=a.samples,warmups=a.warmups,orders=[[16,17],[17,16]],percentile='floor((n-1)*p/100)',limits='Requested heap excludes bookkeeping, stacks and GPU. Both versions execute one verified binary. Scene names refer to historical locations; geometry changes. No physical GPU claim.'),indent=2)+'\n')


if __name__=='__main__':main()
