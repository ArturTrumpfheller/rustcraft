#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Linux CPU/software-renderer comparison with the accepted Slice20 commit.
Reuses the cached Bevy rlib when compiling the baseline fixture directly, keeping
Cargo's current workspace artifacts intact. Requires Linux /proc and a display.
"""
import argparse
import csv
import io
import json
from pathlib import Path
import platform
import os
import re
import shutil
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
BASE = 'ab36403bd5742438499c645d1bdb6e19cd4a1317'

def run(command, **kwargs):
    return subprocess.check_output(command, cwd=ROOT, text=True, **kwargs)

def artifacts(command):
    lines = run(command + ['--message-format=json'])
    result = {}
    for line in lines.splitlines():
        item = json.loads(line)
        if item.get('reason') == 'compiler-artifact':
            for filename in item['filenames']:
                if filename.endswith('.rlib'):
                    result[item['target']['name']] = filename
    return result

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--adapter-note', default='Record the selected adapter/driver separately.')
    args = parser.parse_args()
    assert 1 <= args.samples <= 7
    args.output.mkdir(parents=True, exist_ok=False)
    scratch = ROOT / 'work'
    scratch.mkdir(exist_ok=True)
    current = artifacts(['cargo','build','--locked','-p','rustcraft','--example','day_night_gallery'])
    with tempfile.TemporaryDirectory(prefix='day-night-baseline-', dir=scratch) as temporary:
        baseline = Path(temporary)
        paths = run(['git','ls-tree','-r','--name-only',BASE,'crates/voxel_core/src','client/src','assets/terrain']).splitlines()
        for name in paths:
            dest = baseline / name
            dest.parent.mkdir(parents=True,exist_ok=True)
            dest.write_bytes(subprocess.check_output(['git','show',f'{BASE}:{name}'],cwd=ROOT))
        core = baseline / 'crates/voxel_core'
        (core / 'Cargo.toml').write_text('[package]\nname="voxel_core"\nversion="0.1.0"\nedition="2024"\n[workspace]\n[dependencies]\ncrc32fast="1.5.3"\n')
        (core / 'examples').mkdir()
        shutil.copyfile(ROOT/'crates/voxel_core/examples/day_night_bench.rs',core/'examples/day_night_bench.rs')
        old = artifacts(['cargo','build','--offline','--manifest-path',str(core/'Cargo.toml')])
        fixture = (ROOT/'client/examples/day_night_gallery.rs').read_text().replace('voxel_core::time::WorldTime','ReferenceTime')
        fixture += '\n#[derive(Clone,Copy,Default)]\nstruct ReferenceTime(u32);\nimpl ReferenceTime { fn from_parts(_:u64,step:u32)->Option<Self>{Some(Self(step))} fn advance(&mut self,_:u32){} fn sun_direction(self)->[f32;3]{let a=f64::from(self.0)/72000.0*std::f64::consts::TAU;[a.cos() as f32,a.sin() as f32,0.]} fn moon_direction(self)->[f32;3]{self.sun_direction().map(|v|-v)} }\nimpl AtmosphereUniform { fn at_time(radius:i32,_:ReferenceTime)->Self{Self::for_radius(radius)} }\n'
        source = baseline / 'client/examples/day_night_gallery.rs'
        source.parent.mkdir()
        source.write_text(fixture)
        before = baseline / 'slice20-gallery'
        command = ['rustc','--edition=2024','--crate-name','day_night_gallery',str(source),'-C','debuginfo=1','-o',str(before),'-L',f'dependency={ROOT}/target/debug/deps','-L',f'dependency={core}/target/debug/deps','--extern',f'bevy={current["bevy"]}','--extern',f'voxel_core={old["voxel_core"]}']
        for native in (ROOT/'target/debug/build').glob('*/out'):
            command += ['-L',f'native={native}']
        subprocess.run(command,cwd=ROOT,check=True,env={**os.environ,"CARGO_MANIFEST_DIR":str(ROOT/"client"),"CARGO_PKG_NAME":"rustcraft"})
        subprocess.run(['cargo','build','--offline','--release','--manifest-path',str(core/'Cargo.toml'),'--example','day_night_bench'],cwd=ROOT,check=True)
        subprocess.run(['cargo','build','--locked','--release','-p','voxel_core','--example','day_night_bench'],cwd=ROOT,check=True)
        cores = {'before':core/'target/release/examples/day_night_bench','after':ROOT/'target/release/examples/day_night_bench'}
        for name, executable in cores.items():
            runs = []
            for sample in range(args.samples):
                raw = run([str(executable)])
                (args.output/f'core-{name}-{sample+1}.csv').write_text(raw)
                runs.append(list(csv.DictReader(io.StringIO(raw))))
            with (args.output/f'core-{name}.csv').open('w',newline='') as f:
                writer = csv.DictWriter(f,fieldnames=runs[0][0].keys(),lineterminator='\n')
                writer.writeheader()
                for i, first in enumerate(runs[0]):
                    row = dict(first)
                    for field in ['light_us','mesh_us']:
                        row[field] = f'{statistics.median(float(r[i][field]) for r in runs):.2f}'
                    writer.writerow(row)
        rows = []
        for sample in range(args.samples):
            for radius in [1,2,3]:
                order = [('before',before),('after',ROOT/'target/debug/examples/day_night_gallery')]
                for name, executable in order if sample%2 == 0 else reversed(order):
                    raw_file = args.output/f'frame-{name}-r{radius}-{sample+1}.txt'
                    peak_rss = 0
                    with raw_file.open('w') as output:
                        process = subprocess.Popen([str(executable),'plains',str(radius),'--time=18000','--advance-time','--frames=300'],cwd=ROOT,stdout=output)
                        deadline = time.monotonic()+120
                        while process.poll() is None:
                            try:
                                status = Path(f'/proc/{process.pid}/status').read_text()
                                rss = re.search(r'VmRSS:\s+(\d+)',status)
                                if rss: peak_rss = max(peak_rss,int(rss[1]))
                            except FileNotFoundError:
                                pass
                            if time.monotonic()>deadline:
                                process.kill()
                                process.wait()
                                raise TimeoutError('Renderer timed out')
                            time.sleep(0.01)
                        if process.returncode:
                            raise subprocess.CalledProcessError(process.returncode,executable)
                    raw = raw_file.read_text()
                    row = {'version':name,'radius':radius,'sample':sample+1}
                    row.update(dict(re.findall(r'(\w+)=([\w.]+)',raw)))
                    row['rss_kib'] = peak_rss
                    rows.append(row)
                    print(name,radius,sample+1,row['p50_ms'],row['rss_kib'],flush=True)
        fields = ['version','radius','sample','p50_ms','p95_ms','p99_ms','control_p50_ns','writes','quads','mesh_bytes','light_bytes','meshes','terrain_materials','sky_materials','images','rss_kib']
        with (args.output/'frames.csv').open('w',newline='') as f:
            writer = csv.DictWriter(f,fieldnames=fields,extrasaction='ignore',lineterminator='\n')
            writer.writeheader()
            writer.writerows(rows)
    (args.output/'environment.txt').write_text(platform.platform()+'\n'+run(['rustc','--version'])+'Baseline '+BASE+'\n'+args.adapter_note+'\n960x640; debug renderer, release core benchmark. 5s/120frame warmup, 300 measured frames.\nRSS peak sampled every10ms through Linux /proc.\n')
