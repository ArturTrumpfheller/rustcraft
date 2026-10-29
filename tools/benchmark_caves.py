#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Run identical release cave harnesses on merged Slice26 and current code."""
import argparse
import csv
import io
import os
from pathlib import Path
import statistics
import subprocess


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--scratch',type=Path,default=Path('work/cave-benchmark'))
    parser.add_argument('--repeats',type=int,default=3)
    args=parser.parse_args()
    if args.repeats<1:parser.error('--repeats must be positive')
    root=Path(__file__).resolve().parents[1]
    baseline=args.baseline.resolve();output=args.output.resolve();scratch=args.scratch.resolve()
    if baseline==root:parser.error('baseline must be a separate detached worktree')
    harness=Path('crates/voxel_core/examples/cave_bench.rs')
    destination=baseline/harness;source=(root/harness).read_bytes()
    if destination.exists() and destination.read_bytes()!=source:
        parser.error(f'refusing to overwrite another harness: {destination}; remove the previous measurement-only copy explicitly')
    destination.write_bytes(source)
    output.mkdir(parents=True,exist_ok=True)
    binaries={}
    for name,cwd in [('baseline',baseline),('current',root)]:
        target=scratch/name
        subprocess.run(['cargo','build','--release','--locked','--target-dir',str(target),'-p','voxel_core','--example','cave_bench'],cwd=cwd,check=True)
        binaries[name]=target/('release/examples/cave_bench'+('.exe' if os.name=='nt' else ''))
    rows={};header=None
    for repeat in range(1,args.repeats+1):
        for name,build,version in [('baseline-v6','baseline',6),('current-v6','current',6),('current-v7','current',7)]:
            path=output/f'{name}-{repeat}.txt'
            with path.open('x') as stream:
                subprocess.run([str(binaries[build]),str(version)],stdout=stream,cwd=root,check=True)
            lines=[s for s in path.read_text().splitlines() if not s.startswith('TRAVEL')]
            for row in csv.DictReader(io.StringIO('\n'.join(lines))):
                header=list(row)
                rows.setdefault((name,row['scene']),[]).append(row)
            print(path,flush=True)
    with (output/'summary.csv').open('x',newline='') as stream:
        writer=csv.DictWriter(stream,fieldnames=['revision']+header,lineterminator='\n');writer.writeheader()
        for (name,scene),samples in rows.items():
            row={'revision':name,'scene':scene}
            for key in header:
                if key=='scene':continue
                values=[float(sample[key]) for sample in samples]
                row[key]=f'{statistics.median(values):.2f}'
            writer.writerow(row)


if __name__=='__main__':main()
