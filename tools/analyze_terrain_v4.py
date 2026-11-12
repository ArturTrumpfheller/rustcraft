#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Deterministic geographical diagnostics; images are NOT renderer screenshots.
Radial symmetry and skeletons are sampled proxies, requiring visual review.
"""
import argparse,collections,csv,json,math
from pathlib import Path
from generate_textures import png
D8=[(1,0),(1,1),(0,1),(-1,1),(-1,0),(-1,-1),(0,-1),(1,-1)]
def quantiles(a):
 a=sorted(a);return {str(p):a[(len(a)-1)*p//100] for p in [0,50,95,99,100]} if a else {}
def components(points):
 todo=set(points);out=[]
 while todo:
  start=todo.pop();q=[start];cells=[]
  while q:
   p=q.pop();cells.append(p)
   for dx,dz in D8[::2]:
    n=(p[0]+dx,p[1]+dz)
    if n in todo:todo.remove(n);q.append(n)
  out.append(cells)
 return sorted(out,key=len,reverse=True)
def thin(points):
 # Zhang-Suen thinning on a binary mountain mask, no heightfield fitting.
 p=set(points)
 while True:
  removed=0
  for phase in [0,1]:
   dead=[]
   for x,z in p:
    a=[int((x+dx,z+dz) in p) for dx,dz in D8];n=sum(a)
    if not 2<=n<=6 or sum(a[i]==0 and a[(i+1)%8]==1 for i in range(8))!=1:continue
    # Index 0 east,2 south,4 west,6 north.
    triplets=[(6,0,2),(0,2,4)] if phase==0 else [(6,0,4),(6,2,4)]
    if all(math.prod(a[i] for i in t)==0 for t in triplets):dead.append((x,z))
   p.difference_update(dead);removed+=len(dead)
  if not removed:return p

def analyze(rows,out,name):
 step=16;g={(r['x']//step,r['z']//step):r for r in rows};h={p:r['height'] for p,r in g.items()}
 mountains={p for p,v in h.items() if v>=70};parts=components(mountains);skeleton=thin(mountains);orient=[0]*4;branches=0;length=0
 for x,z in skeleton:
  degree=0
  for k,(dx,dz) in enumerate(D8):
   if (x+dx,z+dz) in skeleton:
    degree+=1
    if k<4:orient[k]+=1;length+=step*math.hypot(dx,dz)
  branches+=degree>=3
 patches=0;patches2=0;patches4=0;eligible=0;radial=0;peaks=0;valleys=[]
 for (x,z),r in g.items():
  if r['height']>=4:
   patch=[h.get((x+dx,z+dz)) for dx in [0,1,2] for dz in [0,1,2]]
   if all(v is not None for v in patch):
    eligible+=1;span=max(patch)-min(patch);patches+=span<=1;patches2+=span<=2;patches4+=span<=4
  if r['height']>=50 and all(h.get((x+dx,z+dz),9999)<r['height'] for dx,dz in D8):
   peaks+=1
   rings=[[h.get((x+dx*s,z+dz*s)) for dx,dz in D8] for s in [2,4,8]]
   if all(all(v is not None for v in ring) for ring in rings):
    means=[sum(ring)/8 for ring in rings];drops=[r['height']-m for m in means]
    radial+=all(d>=4 and math.sqrt(sum((v-m)**2 for v in ring)/8)<d*.22 for ring,m,d in zip(rings,means,drops))
  for dx,dz in [(16,0),(0,16)]:
   if (x-dx,z-dz) in h and (x+dx,z+dz) in h:
    depth=min(h[x-dx,z-dz],h[x+dx,z+dz])-r['height']
    if depth>=20:valleys.append([depth,x*step,z*step,r['height'],dx*step,dz*step])
 broad_extrema=0;broad_radial=0
 for (x,z),v in h.items():
  if x%4 or z%4:continue
  neighbors=[h.get((x+dx,z+dz)) for dx in [-4,-2,0,2,4] for dz in [-4,-2,0,2,4] if dx or dz]
  if any(n is None for n in neighbors):continue
  sign=1 if v>=max(neighbors) and v-min(neighbors)>=4 else (-1 if v<=min(neighbors) and max(neighbors)-v>=4 else 0)
  if not sign:continue
  broad_extrema+=1;rings=[]
  for r in [4,8,16]:
   d=round(r/math.sqrt(2));offsets=[(r,0),(d,d),(0,r),(-d,d),(-r,0),(-d,-d),(0,-r),(d,-d)]
   rings.append([h.get((x+dx,z+dz)) for dx,dz in offsets])
  if any(any(n is None for n in ring) for ring in rings):continue
  means=[sum(ring)/8 for ring in rings];drops=[sign*(v-m) for m in means]
  broad_radial+=all(d>=3 and math.sqrt(sum((n-m)**2 for n in ring)/8)<d*.35 for ring,m,d in zip(rings,means,drops))
 variations={}
 correlations={}
 for scale in [16,64,256,1024]:
  s=scale//step;diff=[abs(v-h[x+s,z]) for (x,z),v in h.items() if (x+s,z) in h];variations[str(scale)]=quantiles(diff)
  pairs=[(v,h[x+s,z]) for (x,z),v in h.items() if (x+s,z) in h]
  if pairs:
   a=sum(v for v,_ in pairs)/len(pairs);b=sum(v for _,v in pairs)/len(pairs)
   denom=math.sqrt(sum((v-a)**2 for v,_ in pairs)*sum((v-b)**2 for _,v in pairs));correlations[str(scale)]=sum((v-a)*(w-b) for v,w in pairs)/denom if denom else 0
 spans=[]
 for part in parts[:12]:
  xs,zs=zip(*part);spans.append({'cells':len(part),'width':(max(xs)-min(xs)+1)*step,'depth':(max(zs)-min(zs)+1)*step})
 result={'seed':rows[0]['seed'],'samples':len(rows),'elevation_quantiles':quantiles(list(h.values())),'one_block_slope_quantiles':quantiles([r['slope'] for r in rows]),'slope_counts':dict(collections.Counter(r['slope'] for r in rows)),'height_difference_quantiles':variations,'x_height_autocorrelation':correlations,'land_flat32_fraction':patches/max(1,eligible),'land_span32_le2_fraction':patches2/max(1,eligible),'land_span32_le4_fraction':patches4/max(1,eligible),'mountain_area_fraction':len(mountains)/len(rows),'mountain_components':spans,'isolated_mountain_components_le_4_samples':sum(len(p)<=4 for p in parts),'ridge_skeleton_length':length,'ridge_skeleton_branch_pixels':branches,'ridge_edge_orientation_counts_E_SE_S_SW':orient,'strict_sampled_peaks':peaks,'radial_peak_candidates_22pct':radial,'broad_64_extrema':broad_extrema,'broad_radial_64_128_256_candidates_35pct':broad_radial,'deepest_512_span_valley':max(valleys) if valleys else None}
 # Central 8192² and two distant windows share the same height/color scales.
 for region,(ox,oz,size) in enumerate([(-4096,-4096,8192),(-24000,20000,2048),(28000,-30000,2048)]):
  n=size//step;rgba=bytearray();gray=bytearray()
  for z in range(oz//step,(oz+size)//step):
   for x in range(ox//step,(ox+size)//step):
    v=h[x,z];east=h.get((x+1,z),v);south=h.get((x,z+1),v);shade=min(1.4,max(.35,1+(v-east)*.065+(v-south)*.035))
    color=(35,85,145) if v<=2 else (min(220,60+v),min(220,110+v*.45),min(220,45+v*.9))
    rgba.extend([min(255,int(c*shade)) for c in color]+[255]);level=min(255,max(0,(v+40)*255//222));gray.extend([level]*3+[255])
  (out/f'{name}-region{region}-relief.png').write_bytes(png(n,n,rgba,compression=9));(out/f'{name}-region{region}-height.png').write_bytes(png(n,n,gray,compression=9))
 return result

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('input',type=Path);p.add_argument('--output',type=Path,required=True);p.add_argument('--version',required=True);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True);groups=collections.defaultdict(list)
 with a.input.open() as f:
  for row in csv.DictReader(f):
   row={k:int(v) for k,v in row.items()};groups[row['seed']].append(row)
 results=[]
 for seed,rows in groups.items():
  results.append(analyze(rows,a.output,f'v{a.version}-seed{seed}'));print(seed,flush=True)
 (a.output/f'quality-v{a.version}.json').write_text(json.dumps(results,indent=2)+'\n')
if __name__=='__main__':main()
