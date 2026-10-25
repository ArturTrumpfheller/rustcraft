#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Matched Linux software/native renderer measurements; run binaries sequentially.
Both binaries use the same solid fixture and atmosphere; Slice21 has no water.
Uses per-child getrusage. Environment selects the desktop/driver; no backend override.
"""
import argparse
import csv
from pathlib import Path
import subprocess
import sys

if len(sys.argv)>1 and sys.argv[1]=="--child":
    import resource
    result=subprocess.run(sys.argv[2:],check=False)
    usage=resource.getrusage(resource.RUSAGE_CHILDREN)
    print(f"WATER_RESOURCE user_s={usage.ru_utime:.4f} system_s={usage.ru_stime:.4f} rss_kib={usage.ru_maxrss}",file=sys.stderr)
    sys.exit(result.returncode)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('before',type=Path)
    p.add_argument('after',type=Path)
    p.add_argument('output',type=Path)
    p.add_argument('--frames',type=int,default=180)
    args=p.parse_args()
    args.output.mkdir(parents=True,exist_ok=True)
    rows=[]
    for scene in ['dry','pool','waterfall','several','boundary']:
        for phase,binary in [('slice21',args.before),('slice22',args.after)]:
            result=subprocess.run([sys.executable,str(Path(__file__).resolve()),'--child',str(binary.resolve()),scene,'2','--time=6000',f'--frames={args.frames}'],capture_output=True,text=True,check=True,timeout=240)
            (args.output/f'{phase}-{scene}.txt').write_text(result.stdout+result.stderr)
            if 'ERROR' in result.stderr:
                raise RuntimeError(result.stderr)
            frame=next(l for l in result.stdout.splitlines() if l.startswith('WATER_FRAME '))
            resources=next(l for l in result.stderr.splitlines() if l.startswith('WATER_RESOURCE '))
            row={'phase':phase,**dict(v.split('=',1) for v in frame.split()[1:]),**dict(v.split('=',1) for v in resources.split()[1:])}
            rows.append(row)
            with (args.output/'frames.csv').open('w',newline='') as f:
                writer=csv.DictWriter(f,fieldnames=list(row),lineterminator="\n");writer.writeheader();writer.writerows(rows)
            print(phase,frame,resources,flush=True)


if __name__=='__main__':
    main()
