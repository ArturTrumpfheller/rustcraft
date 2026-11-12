#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Plot measured Rust height profiles; optional development-only Matplotlib."""
import argparse,csv
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('v10',type=Path);p.add_argument('v11',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 import matplotlib
 matplotlib.use('Agg')
 import matplotlib.pyplot as plt
 fig,axes=plt.subplots(3,1,figsize=(12,8),layout='constrained')
 sections=[(0,-3200,-900,-100,'Valley and asymmetric flanks'),(0,-3152,-4300,-3600,'Range at the Slice35 mountain site'),(2026,-4096,-2900,-2200,'Ordinary plains and transition')]
 for version,path in [(10,a.v10),(11,a.v11)]:
  samples=[[] for _ in sections]
  with path.open() as f:
   for r in csv.DictReader(f):
    seed,x,z,h=map(int,[r['seed'],r['x'],r['z'],r['height']])
    for i,(s,sz,lo,hi,_) in enumerate(sections):
     if s==seed and z==sz and lo<=x<=hi:samples[i].append((x,h))
  for ax,data,section in zip(axes,samples,sections):
   data.sort();ax.plot([p[0] for p in data],[p[1] for p in data],label=f'v{version}',linewidth=1.5)
   ax.set_title(f'{section[4]} · seed {section[0]} · Z {section[1]}');ax.set_xlabel('World X (blocks)');ax.set_ylabel('Surface Y');ax.set_ylim(-35,180);ax.grid(alpha=.2);ax.legend()
 fig.suptitle('Measured terrain profiles · 16-block samples · diagnostic, not a renderer capture')
 a.output.parent.mkdir(parents=True,exist_ok=True);fig.savefig(a.output,dpi=140)
if __name__=='__main__':main()
