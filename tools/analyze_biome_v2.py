#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Summarize the actual Q10 surface survey. Maps are diagnostics, not renders."""
import argparse,collections,csv,gzip,json,os
from pathlib import Path
os.environ.setdefault('MPLCONFIGDIR',str(Path('work/matplotlib').resolve()))
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import ListedColormap
from matplotlib.patches import Patch
NAMES=['Plains','Forest','Desert','Snowy Plains','Rocky Highlands','Meadow','Birch Forest','Taiga','Grove','Snowy Slopes','Stony Peaks','Beach','Ocean','River','Lake']
COLORS=['#a8c964','#246b3f','#dfc17a','#dce8e9','#858579','#79b75d','#68a255','#356b67','#709998','#f6f5f0','#a9a09b','#efdbab','#387eae','#48b8ce','#75c7d1']
def percentile(v,p):return sorted(v)[(len(v)-1)*p//100] if v else 0
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('survey',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
 rows=list(csv.DictReader(a.survey.open()));groups=collections.defaultdict(list)
 for r in rows:
  for k in r:
   if k not in ['region']:r[k]=int(r[k])
  groups[(r['seed'],r['region'])].append(r)
 stats=[];freq=[];adjacency=collections.Counter();sites=[];component_rows=[]
 for (seed,region),rs in groups.items():
  xs=sorted({r['x'] for r in rs});zs=sorted({r['z'] for r in rs});step=xs[1]-xs[0];nx=len(xs);nz=len(zs)
  grid=np.empty((nz,nx),dtype=np.int16);heights=np.empty_like(grid);lookup={}
  for r in rs:
   x=(r['x']-xs[0])//step;z=(r['z']-zs[0])//step;grid[z,x]=r['biome'];heights[z,x]=r['height'];lookup[(x,z)]=r
  components=collections.defaultdict(list);seen=set();singletons=0
  for z in range(nz):
   for x in range(nx):
    if (x,z) in seen:continue
    b=int(grid[z,x]);todo=[(x,z)];seen.add((x,z));n=0;edge=False
    while todo:
     cx,cz=todo.pop();n+=1;edge|=cx in [0,nx-1] or cz in [0,nz-1]
     for k in [(cx-1,cz),(cx+1,cz),(cx,cz-1),(cx,cz+1)]:
      dx,dz=k
      if 0<=dx<nx and 0<=dz<nz and k not in seen and grid[dz,dx]==b:seen.add(k);todo.append(k)
    components[b].append((n,edge));singletons+=n==1 and not edge
  boundary=0
  for z in range(nz):
   for x in range(nx):
    for dx,dz in [(1,0),(0,1)]:
     if x+dx<nx and z+dz<nz and grid[z,x]!=grid[z+dz,x+dx]:
      boundary+=step;adjacency[tuple(sorted((int(grid[z,x]),int(grid[z+dz,x+dx]))))]+=1
  stats.append(dict(seed=seed,region=region,step=step,cells=len(rs),area_blocks=len(rs)*step*step,boundary_blocks=boundary,boundary_per_1024_area=boundary*1024/(len(rs)*step*step),interior_single_cell_components=singletons,single_cell_fraction=singletons/len(rs)))
  for b in range(15):
   br=[r for r in rs if r['biome']==b];sizes=[n*step*step for n,e in components[b]];interior=[n*step*step for n,e in components[b] if not e]
   if not br:continue
   ordered=sorted(sizes);total=sum(ordered);cumulative=0;weighted_median=ordered[-1]
   for size in ordered:
    cumulative+=size
    if cumulative*2>=total:weighted_median=size;break
   component_rows.extend(dict(seed=seed,region=region,biome=NAMES[b],samples=n,area_blocks=n*step*step,touches_survey_edge=int(e)) for n,e in components[b])
   freq.append(dict(seed=seed,region=region,biome=NAMES[b],area_weighted_median=weighted_median,area_weighted_mean=sum(v*v for v in ordered)/total,cells=len(br),percent=100*len(br)/len(rs),components=len(sizes),median_component_area=percentile(sizes,50),p95_component_area=percentile(sizes,95),max_component_area=max(sizes),interior_components=len(interior),median_height=percentile([r['height'] for r in br],50),p95_height=percentile([r['height'] for r in br],95),median_slope=percentile([r['slope'] for r in br],50),p95_slope=percentile([r['slope'] for r in br],95)))
   if region=='central':
    candidates=[(x,z) for (x,z),r in lookup.items() if r['biome']==b]
    def score(pos):
     x,z=pos;return sum(0<=x+dx<nx and 0<=z+dz<nz and grid[z+dz,x+dx]==b for dx in [-2,-1,0,1,2] for dz in [-2,-1,0,1,2])*10-lookup[pos]['slope']*4
    best=max(candidates,key=score);sites.append({**lookup[best],'biome_name':NAMES[b],'interior_score':score(best)})
  if region=='central':
   fig,axes=plt.subplots(1,2,figsize=(15,7));extent=[xs[0],xs[-1]+step,zs[-1]+step,zs[0]]
   axes[0].imshow(grid,cmap=ListedColormap(COLORS),vmin=-.5,vmax=14.5,extent=extent,interpolation='nearest');axes[0].set_title('Surface biome, actual water + v11 geography')
   im=axes[1].imshow(heights,cmap='terrain',vmin=-32,vmax=160,extent=extent,interpolation='nearest');axes[1].set_title('Unchanged v11/v12 surface elevation');fig.colorbar(im,ax=axes[1],shrink=.6,label='Surface block Y')
   for ax in axes:ax.set_xlabel('Absolute X (blocks)');ax.set_ylabel('Absolute Z (blocks)')
   fig.suptitle(f'RustCraft generator12 — seed {seed} — {step}-block survey (diagnostic map)')
   fig.legend(handles=[Patch(color=c,label=n) for c,n in zip(COLORS,NAMES)],loc='lower center',ncol=8,fontsize=8);fig.tight_layout(rect=(0,.09,1,.96));fig.savefig(a.output/f'biomes-{seed}.png',dpi=140);plt.close(fig)
 for name,data in [('regions',stats),('frequency',freq),('sites',sites),('adjacency',[dict(a=NAMES[a],b=NAMES[b],edges=n) for (a,b),n in sorted(adjacency.items())])]:
  with (a.output/f'{name}.csv').open('w',newline='') as f:
   w=csv.DictWriter(f,list(data[0]),lineterminator='\n');w.writeheader();w.writerows(data)
 with gzip.open(a.output/'components.csv.gz','wt',newline='') as f:
  w=csv.DictWriter(f,list(component_rows[0]),lineterminator='\n');w.writeheader();w.writerows(component_rows)
 (a.output/'method.json').write_text(json.dumps(dict(input=str(a.survey),samples=len(rows),connectivity='4-neighbor center samples; edge-touching components censored separately',area='Center-sample region estimate, not exhaustive per-block biome polygon area',singletons='Interior one-sample connected components; at step16 a one-chunk center-sample proxy, not a proof that every block in a chunk shares its biome'),indent=2)+'\n')
 print(f'{len(rows)} samples; {len(groups)} seed/regions; {len(freq)} biome summaries')
if __name__=='__main__':main()
