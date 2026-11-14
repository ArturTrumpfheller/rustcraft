#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Seed-cluster uncertainty and static figures for common cave-volume analysis."""
import argparse
import hashlib
import json
import os
from pathlib import Path

os.environ.setdefault('MPLCONFIGDIR','/tmp/rustcraft-matplotlib')
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import ListedColormap
import numpy as np

from analyze_cave_volumes import load


def interval(rows, numerator, denominator, scale=1):
    seeds=sorted({r['seed'] for r in rows})
    sums=np.array([[sum(numerator(r) for r in rows if r['seed']==s),
                    sum(denominator(r) for r in rows if r['seed']==s)] for s in seeds],float)
    rng=np.random.default_rng(392026)
    boots=sums[rng.integers(0,len(sums),(4096,len(sums)))].sum(axis=1)
    values=scale*boots[:,0]/np.maximum(boots[:,1],1)
    return dict(estimate=float(scale*sums[:,0].sum()/max(1,sums[:,1].sum())),
                low=float(np.percentile(values,2.5)),high=float(np.percentile(values,97.5)),
                seed_clusters=len(seeds),windows=len(rows),
                zero_observations=bool((sums[:,0]==0).all()))


def aggregate(rows):
    return dict(
        cavity_percent=interval(rows,lambda r:r['cavity'],lambda r:r['geology'],100),
        dry_air_percent=interval(rows,lambda r:r['dry_air'],lambda r:r['geology'],100),
        near_solid_window_percent=interval(rows,lambda r:r['cavity']<64,lambda r:1,100),
        passable_portals_per_km2=interval(rows,lambda r:r['entrance_portal_clusters'],lambda r:r['n']**2,1e6),
        encountered_portals_per_1000_walked_blocks=interval(rows,lambda r:r['walked_portal_clusters'],lambda r:r['surface_walk_block_length'],1000),
        blocked_surface_walk_percent=interval(rows,lambda r:r['blocked_surface_walks'],lambda r:4,100),
        mean_largest_geometry_component=interval(rows,lambda r:r['largest'],lambda r:1),
        mean_largest_standing_component=interval(rows,lambda r:r['largest_standing'],lambda r:1),
        mean_geometry_route_lower_bound=interval(rows,lambda r:r['geometry_geodesic_lower_bound'],lambda r:1),
        mean_standing_route_lower_bound=interval(rows,lambda r:r['standing_geodesic_lower_bound'],lambda r:1),
        total_walked_blocks=sum(r['surface_walk_block_length'] for r in rows),
        cavity_window_percentiles=dict(zip(['min','p50','p95','p99','max'],map(float,np.percentile(
            [r['cavity_fraction']*100 for r in rows],[0,50,95,99,100])))),
        normalized_geology=np.array([r['normalized_geology'] for r in rows]).sum(axis=0).tolist(),
        normalized_cavity=np.array([r['normalized_cavity'] for r in rows]).sum(axis=0).tolist(),
        normalized_depth_percent=[interval(rows,lambda r:r['normalized_cavity'][i],lambda r:r['normalized_geology'][i],100) for i in range(10)],
        width_bins=np.array([r['width_bins'] for r in rows]).sum(axis=0).tolist(),
        headroom_bins=np.array([r['headroom_bins'] for r in rows]).sum(axis=0).tolist(),
        broad_core_bins=np.array([r['broad_core_sizes'] for r in rows]).sum(axis=0).tolist(),
        component_bins=np.array([r['component_bins'] for r in rows]).sum(axis=0).tolist())


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--arm',action='append',required=True,help='LABEL=analysis.jsonl')
    p.add_argument('--volumes',action='append',default=[],help='LABEL=directory, for fixed section figures')
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--site',action='append',help='Fixed section window ID; may be repeated')
    a=p.parse_args()
    arms={label:Path(path) for label,path in (s.split('=',1) for s in a.arm)}
    volumes={label:Path(path) for label,path in (s.split('=',1) for s in a.volumes)}
    a.output.mkdir(parents=True,exist_ok=True)
    summary={}
    all_rows={}
    for label,path in arms.items():
        rows=[json.loads(line) for line in path.read_text().splitlines()]
        all_rows[label]=rows
        assert len({r['id'] for r in rows})==len(rows)
        strata=[
            ('unconditional',lambda r:r['stratum'].endswith('-unconditional')),
            ('lowland-conditional',lambda r:r['stratum'].startswith('lowland-')),
            ('mountain-conditional',lambda r:r['stratum'].startswith('mountain-'))]
        summary[label]={key:aggregate(group) for key,predicate in strata
                        if (group:=[r for r in rows if predicate(r)])}
        assert 'unconditional' in summary[label], 'each arm needs unconditional sample windows'
    (a.output/'summary.json').write_text(json.dumps(summary,indent=2,allow_nan=False)+'\n')
    fig,axes=plt.subplots(1,3,figsize=(15,4.5),layout='constrained')
    for label in arms:
        d=summary[label]['unconditional']['normalized_depth_percent']
        x=np.arange(10)*.1+.05
        axes[0].plot(x,[v['estimate'] for v in d],label=label)
        axes[0].fill_between(x,[v['low'] for v in d],[v['high'] for v in d],alpha=.12)
        for ax,key in zip(axes[1:],['width_bins','headroom_bins']):
            v=np.array(summary[label]['unconditional'][key],float)
            ax.plot(range(7),v/max(1,v.sum())*100,marker='o',label=label)
    axes[0].set(xlabel='Depth / local geological thickness',ylabel='Cavity / geological volume (%)',title='Unconditional windows; seed-cluster 95% intervals')
    for ax,title in zip(axes[1:],['Minimum horizontal run','Vertical run / headroom']):
        ax.set(xticks=range(7),xticklabels=['<2','2–3','4–7','8–15','16–31','32–63','64+'],
               xlabel='Contiguous voxel run (blocks)',ylabel='Fraction of cavity voxels (%)',title=title)
    axes[0].legend()
    for ax in axes:ax.grid(alpha=.2)
    fig.savefig(a.output/'depth-and-size.png',dpi=150)
    plt.close(fig)
    fig,axes=plt.subplots(1,3,figsize=(14,4.5),layout='constrained')
    labels=list(arms)
    for ax,key,title in zip(axes,['cavity_percent','mean_largest_standing_component','passable_portals_per_km2'],
            ['Cavity volume (%)','Largest supported floor component (nodes)','Passable portal clusters / km²']):
        for i,stratum in enumerate(['unconditional','lowland-conditional','mountain-conditional']):
            present=[j for j,label in enumerate(labels) if stratum in summary[label]]
            values=[summary[labels[j]][stratum][key] for j in present]
            x=np.array(present)+(i-1)*.22
            mean=np.array([v['estimate'] for v in values])
            ax.bar(x,mean,width=.21,label=stratum,yerr=[mean-[v['low'] for v in values],[v['high'] for v in values]-mean],capsize=2)
        ax.set(xticks=range(len(labels)),xticklabels=labels,title=title)
        ax.grid(axis='y',alpha=.2)
    axes[0].legend(fontsize=7)
    fig.savefig(a.output/'regional-comparison.png',dpi=150)
    plt.close(fig)
    cmap=ListedColormap(['#59636b','#f5f6f8','#409ed4','#d75b34','#b39b73'])
    for site in (a.site or ['s00-0','s01-4','s01-6']):
        if not volumes:break
        fig,axes=plt.subplots(len(volumes),3,figsize=(12,3.4*len(volumes)),squeeze=False,layout='constrained')
        for row,(label,directory) in enumerate(volumes.items()):
            cells,surface,meta=load(directory/(site+'.rcv'))
            n,h=cells.shape[0],cells.shape[2]
            y=np.arange(h)[None,None,:]+meta['min_y']
            under=(y>=meta['min_y']+4)&(y<=surface[:,:,None]-8)
            cavity=((cells==1)|(cells==2)|(cells==3))&under
            fraction=cavity.sum(axis=2)/np.maximum(1,under.sum(axis=2))*100
            im=axes[row,0].imshow(fraction,origin='lower',vmin=0,vmax=50,cmap='viridis',extent=(0,n,0,n))
            fig.colorbar(im,ax=axes[row,0],label='Cavity / column geology (%)')
            axes[row,0].set_title(f'{label}: density map')
            yy=np.clip(((surface-(meta['min_y']+4))*.5+(meta['min_y']+4)-meta['min_y']).astype(int),0,h-1)
            cut=np.take_along_axis(cells,yy[:,:,None],axis=2)[:,:,0]
            axes[row,1].imshow(cut,origin='lower',cmap=cmap,vmin=0,vmax=4,extent=(0,n,0,n))
            axes[row,1].set_title('Horizontal section: 50% relative depth')
            axes[row,2].imshow(cells[n//2].T,origin='lower',cmap=cmap,vmin=0,vmax=4,
                               extent=(0,n,meta['min_y'],meta['min_y']+h),aspect='auto')
            axes[row,2].plot(np.arange(n),surface[n//2],color='#d9ad45',linewidth=.8)
            axes[row,2].set_title('Vertical section: fixed middle Z')
            for ax in axes[row]:ax.set_xlabel('Local X (blocks)')
        fig.savefig(a.output/f'sections-{site}.png',dpi=140)
        plt.close(fig)
    (a.output/'report-method.json').write_text(json.dumps(dict(
        analysis_sha256=hashlib.sha256((Path(__file__).parent/'analyze_cave_volumes.py').read_bytes()).hexdigest(),
        report_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        inputs={label:hashlib.sha256(path.read_bytes()).hexdigest() for label,path in arms.items()},
        bootstrap='4096 resamples of whole seed clusters; RNG392026; percentile95%; windows/voxels are not independent trials',
        caveats=['Conditional terrain strata are separate, not population weights.',
                 'All-zero bootstrap intervals are degenerate observed-sample intervals, not upper population bounds.',
                 'This is a fixed tuning/evaluation cohort, not held-out validation of distributional parity.',
                 'Voxel runs/broad cores are shape proxies, not unique physical tunnels/chambers.',
                 'Window-clipped geodesics are two-sweep lower bounds.',
                 'Surface walks use conservative floor graphs and count unreachable endpoint pairs.',
                 'No Minecraft measurements exist unless a genuine reference arm is supplied.']),indent=2)+'\n')


if __name__=='__main__':
    main()
