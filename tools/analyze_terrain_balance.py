#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Multi-scale geography diagnostics, never renderer screenshots.
Requires analysis-only numpy/scipy/matplotlib. Thresholds are descriptive
RustCraft categories, not measured Minecraft targets. All regions use spacing16.
"""
import argparse,json,os
from pathlib import Path
os.environ.setdefault('MPLCONFIGDIR','/tmp/rustcraft-mpl')
import numpy as np
from scipy.ndimage import maximum_filter,minimum_filter,label
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

def quantile(a):
 return dict(zip(['p0','p50','p95','p99','max'],map(float,np.percentile(a,[0,50,95,99,100])))) if a.size else {}
def parts(mask,step):
 labs,n=label(mask);sizes=np.bincount(labs.ravel())[1:];sizes.sort()
 return dict(count=int(n),largest_areas_blocks2=[int(v)*step**2 for v in sizes[-10:][::-1]],median_area_blocks2=float(np.median(sizes)*step**2) if n else 0)
def analyze(path,out,version,step):
 data=np.genfromtxt(path,delimiter=',',names=True,dtype=['U20']+['i8']*11,encoding='utf8');result=[]
 for seed in np.unique(data['seed']):
  for region in np.unique(data['region']):
   rows=data[(data['seed']==seed)&(data['region']==region)];n=int(np.sqrt(len(rows)));assert n*n==len(rows)
   h=rows['height'].reshape(n,n);slope=rows['slope'].reshape(n,n);land=h>2;metrics={};reliefs={}
   for span in [16,32,64,128,256,512]:
    r=span//step
    if r<1:continue
    d=np.concatenate([np.abs(h[:,r:]-h[:,:-r]).ravel(),np.abs(h[r:,:]-h[:-r,:]).ravel()]);metrics[str(span)]=quantile(d)
    # Centered square window spans +/-span/2, inclusive lattice endpoints.
    size=r+1;reliefs[span]=maximum_filter(h,size=size,mode='nearest')-minimum_filter(h,size=size,mode='nearest')
   # Independent of absolute elevation; exclude sea and boundaries lacking context.
   valid=land.copy();edge=256//step;valid[:edge]=False;valid[-edge:]=False;valid[:,:edge]=False;valid[:,-edge:]=False
   low=(reliefs[128]<=12)&(reliefs[256]<=24)&(slope<=1)&valid
   mountain=((reliefs[128]>=40)|((reliefs[256]>=64)&(reliefs[64]>=20)))&valid
   hill=valid&~low&~mountain
   valleys=[]
   # Two opposing shoulders, valley half-depth width on fixed cardinal transects.
   # Bounded 256-wide span; no claim of watershed or off-axis valley recovery.
   for grid in [h,h.T]:
    r=128//step
    for z in range(0,n,4):
     for x in range(r,n-r):
      v=grid[z,x];left=grid[z,x-r:x];right=grid[z,x+1:x+r+1];depth=int(min(left.max(),right.max())-v)
      if depth<12 or v>grid[z,x-1] or v>grid[z,x+1]:continue
      half=v+depth/2;li=np.flatnonzero(left>=half);ri=np.flatnonzero(right>=half)
      if len(li) and len(ri):valleys.append((depth,int((r-li[-1]+ri[0]+1)*step)))
   v=np.array(valleys).reshape(-1,2);count=max(1,int(valid.sum()))
   r=dict(seed=str(seed),region=int(region),samples=len(rows),land_context_samples=int(valid.sum()),elevation=quantile(h),one_block_slope=quantile(slope),height_differences=metrics,relief={str(k):quantile(vv[valid]) for k,vv in reliefs.items()},low_relief_fraction=float(low.sum()/count),hills_fraction=float(hill.sum()/count),mountain_fraction=float(mountain.sum()/count),low_regions=parts(low,step),mountain_regions=parts(mountain,step),cliff_ge3_fraction=float(((slope>=3)&valid).sum()/count),valley_count=len(valleys),valley_depth=quantile(v[:,0]),valley_half_depth_width=quantile(v[:,1]),narrow_deep_valleys=int(((v[:,0]>=24)&(v[:,1]<=48)).sum()),biomes={str(int(b)):int((rows['biome']==b).sum()) for b in np.unique(rows['biome'])})
   result.append(r)
   if step==16 and region==0:
    fig,axes=plt.subplots(1,3,figsize=(15,5),constrained_layout=True);extent=[int(rows['x'].min()),int(rows['x'].max()),int(rows['z'].max()),int(rows['z'].min())]
    for ax,img,title,cmap,lo,hi in [(axes[0],h,'density surface Y','terrain',-32,160),(axes[1],reliefs[128],'128-block local relief','magma',0,100),(axes[2],low.astype(int)+mountain.astype(int)*2,'gentle=1 / rugged=2','viridis',0,2)]:
     im=ax.imshow(img,extent=extent,cmap=cmap,vmin=lo,vmax=hi);ax.set_title(title);fig.colorbar(im,ax=ax,shrink=.65)
    fig.suptitle(f'v{version} seed{seed} — diagnostic maps (not renders)');fig.savefig(out/f'v{version}-seed{seed}-maps.png',dpi=120);plt.close(fig)
    fig,axes=plt.subplots(3,1,figsize=(14,6),sharex=True,sharey=True,constrained_layout=True)
    for ax,z in zip(axes,[n//4,n//2,3*n//4]):ax.plot(rows['x'][:n],h[z],lw=.8);ax.set_ylabel(f'Y / Z={rows["z"][z*n]}');ax.set_ylim(-35,175);ax.grid(alpha=.2)
    fig.suptitle(f'v{version} seed{seed} — fixed full-window transects');fig.savefig(out/f'v{version}-seed{seed}-sections.png',dpi=120);plt.close(fig)
 return result
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('input',type=Path);p.add_argument('--output',type=Path,required=True);p.add_argument('--generator',required=True);p.add_argument('--step',type=int,default=16);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
 result=analyze(a.input,a.output,a.generator,a.step);(a.output/f'geography-v{a.generator}.json').write_text(json.dumps(result,indent=2)+'\n')
 for r in result:print(r['seed'],r['region'],*[round(r[k],4) for k in ['low_relief_fraction','hills_fraction','mountain_fraction','cliff_ge3_fraction']],r['narrow_deep_valleys'])
