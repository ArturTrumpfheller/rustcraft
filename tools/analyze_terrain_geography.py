#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Shared grid-only diagnostics for actual Horizon and RustCraft terrain.

No renderer images, target quotas or Minecraft biome frequency claims. Heights
in the input are sea-offset (reference Y-61); normalized reference scales the
257-block above-sea span to RustCraft's190. Measures both, separately.
"""
import argparse
import json
import os
from pathlib import Path
os.environ.setdefault('MPLCONFIGDIR', '/tmp/rustcraft-mpl')
import numpy as np
from scipy.ndimage import maximum_filter, minimum_filter, label, binary_fill_holes, find_objects
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from analyze_terrain_balance import quantile, parts


def metrics(h, step):
    h = np.asarray(h, dtype=float)
    relief, differences, slopes, roughness = {}, {}, {}, {}
    valid = h > 2
    edge = 256 // step
    if edge:
        valid[:edge] = False
        valid[-edge:] = False
        valid[:, :edge] = False
        valid[:, -edge:] = False
    for span in [16, 32, 64, 128, 256, 512]:
        r = span // step
        if r < 1:
            continue
        delta = np.concatenate([np.abs(h[:, r:] - h[:, :-r]).ravel(), np.abs(h[r:, :] - h[:-r, :]).ravel()])
        differences[str(span)] = quantile(delta)
        slopes[str(span)] = quantile(delta / span)
        relief[span] = maximum_filter(h, size=r + 1, mode='nearest') - minimum_filter(h, size=r + 1, mode='nearest')
        # Curvature-like high-pass residual at a declared physical scale, in Y.
        if 2 * r < h.shape[0]:
            residual = h[r:-r, r:-r] - (h[:-2*r, r:-r] + h[2*r:, r:-r] + h[r:-r, :-2*r] + h[r:-r, 2*r:]) / 4
            roughness[str(span)] = quantile(np.abs(residual))
    low = (relief[128] <= 12) & (relief[256] <= 24) & valid
    mountain = ((relief[128] >= 40) | ((relief[256] >= 64) & (relief[64] >= 20))) & valid
    hills = valid & ~low & ~mountain
    count = max(1, int(valid.sum()))
    valleys = []
    r = 128 // step
    for grid in [h, h.T]:
        for row in grid[::4]:
            for x in range(r, len(row) - r):
                floor = row[x]
                depth = min(row[x-r:x].max(), row[x+1:x+r+1].max()) - floor
                if depth < 12 or floor > row[x-1] or floor > row[x+1]:
                    continue
                left = np.flatnonzero(row[x-r:x] >= floor + depth/2)
                right = np.flatnonzero(row[x+1:x+r+1] >= floor + depth/2)
                if len(left) and len(right):
                    valleys.append((depth, (r-left[-1]+right[0]+1)*step))
    valleys = np.asarray(valleys).reshape(-1, 2)
    # High-elevation components and enclosed low pockets are only screening
    # diagnostics. Circularity cannot establish a repeated-template mechanism.
    high = (h >= 62) & valid
    labs, n = label(high)
    shapes = []
    for i,box in enumerate(find_objects(labs),1):
        if box is None:
            continue
        mask = labs[box] == i
        if mask.sum() < 8:
            continue
        yy, xx = np.nonzero(mask)
        perimeter = np.count_nonzero(mask[:, 1:] != mask[:, :-1]) + np.count_nonzero(mask[1:, :] != mask[:-1, :])
        perimeter += int(mask[0].sum()+mask[-1].sum()+mask[:, 0].sum()+mask[:, -1].sum())
        edge = any(b.start==0 or b.stop==h.shape[k] for k,b in enumerate(box))
        shapes.append(dict(area_blocks2=int(mask.sum())*step**2, axis_ratio=float(max(np.ptp(xx)+1, np.ptp(yy)+1)/min(np.ptp(xx)+1, np.ptp(yy)+1)), compactness=float(4*np.pi*mask.sum()/max(1,perimeter**2)), edge_censored=edge))
    return dict(samples=int(h.size), contextual_land_samples=int(valid.sum()), height=quantile(h), sea_relative_height=quantile(h-2), ocean_fraction=float((h < 2).mean()), height_differences=differences, secant_slopes=slopes, high_pass_roughness=roughness, relief={str(k):quantile(v[valid]) for k,v in relief.items()}, gentle_fraction=float(low.sum()/count), hills_fraction=float(hills.sum()/count), rugged_fraction=float(mountain.sum()/count), gentle_regions=parts(low,step), rugged_regions=parts(mountain,step), valley_count=len(valleys), valley_depth=quantile(valleys[:,0]), valley_half_depth_width=quantile(valleys[:,1]), narrow_deep_minima=int(((valleys[:,0]>=24)&(valleys[:,1]<=48)).sum()), high_components=sorted(shapes,key=lambda r:-r['area_blocks2'])[:20], enclosed_low_area_blocks2=int((binary_fill_holes(high)&~high).sum())*step**2), (low, mountain, relief[128])


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('input',type=Path)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--label',required=True)
    p.add_argument('--step',type=int,default=32,choices=[16,32,64])
    p.add_argument('--horizon',action='store_true')
    a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
    data=np.genfromtxt(a.input,delimiter=',',names=True,dtype=['U20']+['i8']*11,encoding='utf8')
    records=[]
    for seed in np.unique(data['seed']):
        for region in np.unique(data['region']):
            rows=data[(data['seed']==seed)&(data['region']==region)]
            n=int(np.sqrt(len(rows)));assert n*n==len(rows)
            assert np.all(np.diff(rows['x'][:n])==a.step)
            raw=rows['height'].reshape(n,n)
            native,masks=metrics(raw,a.step)
            normalized=metrics(2+(raw-2)*190/257,a.step)[0] if a.horizon else native
            record=dict(seed=str(seed),region=int(region),native_sea_offset=native,normalized_above_sea=normalized)
            if not a.horizon:
                record['one_block_slope']=quantile(rows['slope'])
                record['biomes']={str(int(b)):int((rows['biome']==b).sum()) for b in np.unique(rows['biome'])}
            records.append(record)
            if region != 0:
                continue
            low,rugged,relief=masks
            extent=[rows['x'].min(),rows['x'].max(),rows['z'].max(),rows['z'].min()]
            gx,gz=np.gradient(raw.astype(float),a.step)
            fig,axes=plt.subplots(2,2,figsize=(12,10),constrained_layout=True)
            for ax,img,title,cmap,lo,hi in [
                (axes[0,0],raw,'height (Horizon Y−61 where applicable)','terrain',-32,170),
                (axes[0,1],np.maximum(np.abs(gx),np.abs(gz)),f'{a.step}-block secant slope','magma',0,2),
                (axes[1,0],relief,'128-block relief','magma',0,100),
                (axes[1,1],low.astype(int)+rugged.astype(int)*2,'geometric classes: gentle1 / rugged2','viridis',0,2)]:
                im=ax.imshow(img,extent=extent,cmap=cmap,vmin=lo,vmax=hi);ax.set_title(title);fig.colorbar(im,ax=ax,shrink=.7)
            fig.suptitle(f'{a.label} seed{seed} — measured geometry, not gameplay')
            fig.savefig(a.output/f'{a.label}-seed{seed}-maps.png',dpi=110);plt.close(fig)
            fig,axes=plt.subplots(3,1,figsize=(13,6),sharex=True,sharey=True,constrained_layout=True)
            for ax,z in zip(axes,[n//4,n//2,3*n//4]):
                ax.plot(rows['x'][:n],raw[z],lw=.8);ax.set_ylabel(f'Y / Z={rows["z"][z*n]}');ax.set_ylim(-40,200);ax.grid(alpha=.2)
            fig.suptitle(f'{a.label} seed{seed} — fixed transects (reference Y−61)')
            fig.savefig(a.output/f'{a.label}-seed{seed}-sections.png',dpi=110);plt.close(fig)
    (a.output/f'{a.label}.json').write_text(json.dumps(dict(label=a.label,step=a.step,reference_normalization='Y−61, then sea-relative *190/257' if a.horizon else 'native Y, above-sea span190',records=records),indent=2)+'\n')
    for r in records:
        m=r['native_sea_offset'];print(r['seed'],r['region'],*[round(m[k],4) for k in ['gentle_fraction','hills_fraction','rugged_fraction']],m['narrow_deep_minima'])


if __name__=='__main__':main()
