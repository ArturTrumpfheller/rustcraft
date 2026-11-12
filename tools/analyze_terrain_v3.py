#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Summarize terrain_quality CSV. Connectedness is a sampled four-neighbor proxy,
not a voxel connectivity proof. No generated visualization is a client capture.
"""
import argparse,collections,csv,json
from pathlib import Path
def analyze(rows):
 g={(r['x'],r['z']):r for r in rows};step=min(abs(rows[i+1]['x']-rows[i]['x']) for i in range(len(rows)-1) if rows[i+1]['z']==rows[i]['z'])
 heights=sorted(r['height'] for r in rows)
 def components(pred):
  todo={p for p,r in g.items() if pred(r)};sizes=[]
  while todo:
   q=[todo.pop()];n=0
   while q:
    x,z=q.pop();n+=1
    for p in [(x-step,z),(x+step,z),(x,z-step),(x,z+step)]:
     if p in todo:todo.remove(p);q.append(p)
   sizes.append(n)
  return sorted(sizes,reverse=True)
 flats=[];valleys=[]
 for r in rows:
  x,z,h=r['x'],r['z'],r['height']
  points=[(x+dx,z+dz) for dx in (0,16,32) for dz in (0,16,32)]
  if 4<=h<=25 and all(p in g for p in points):
   patch=[g[p]['height'] for p in points]
   if max(patch)-min(patch)<=1:flats.append(r)
  for dx,dz in [(256,0),(0,256)]:
   if (x+dx,z+dz) in g and (x-dx,z-dz) in g:
    depth=min(g[(x+dx,z+dz)]['height'],g[(x-dx,z-dz)]['height'])-h
    if depth>=20:valleys.append([depth,x,z,h,dx,dz])
 gentle=lambda r:4<=r['height']<=25 and r['slope']<=1 and abs(r['east16']-r['height'])<=1 and abs(r['north16']-r['height'])<=1
 return dict(seed=rows[0]['seed'],n=len(rows),step=step,elevations={k:heights[(len(heights)-1)*q//100] for k,q in [('min',0),('p50',50),('p95',95),('p99',99),('max',100)]},slope_counts=dict(sorted(collections.Counter(r['slope'] for r in rows).items())),mountain_pct=sum(h>=70 for h in heights)*100/len(rows),mountain_components=components(lambda r:r['height']>=70)[:5],gentle_pct=sum(gentle(r) for r in rows)*100/len(rows),gentle_components=components(gentle)[:5],flat32_count=len(flats) if step==16 else None,flat32_pct=len(flats)*100/len(rows) if step==16 else None,peak=max(rows,key=lambda r:r['height']),deepest_512_span_valley=max(valleys) if valleys else None)
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('input',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 groups=collections.defaultdict(list)
 with a.input.open() as f:
  for r in csv.DictReader(f):groups[int(r['seed'])].append({k:int(v) for k,v in r.items()})
 results=[analyze(r) for r in groups.values()];a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(results,indent=2)+'\n')
 print(a.output)
if __name__=='__main__':main()
