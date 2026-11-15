#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Summarize auditable exposure/paired-geometry exports, retaining cohort splits."""
import argparse, collections, gzip, json
from pathlib import Path
import numpy as np


def read(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def distribution(values):
    return dict(n=len(values), **{f'p{p}':float(np.percentile(values,p)) if len(values) else None for p in [50,95,99]})


def surface(rows):
    mouths=[m for r in rows for m in r['mouths'] if m['two_block'] or m['penetration']>=3]
    groups=[]
    nearest=[]
    for row in rows:
        chosen=[m for m in row['mouths'] if m['two_block'] or m['penetration']>=3]
        systems=collections.Counter(i for m in chosen for i in m['systems'])
        groups.extend(systems.values())
        if len(chosen)>1:
            centers=np.array([m['center'][:2] for m in chosen])
            distance=np.sqrt(((centers[:,None,:]-centers[None,:,:])**2).sum(axis=2))
            np.fill_diagonal(distance,np.inf)
            nearest.extend(distance.min(axis=1).tolist())
    area=sum(r['opening_area_m2'] for r in rows)/1e6
    counts={key:sum(r[key] for r in rows) for key in [
        'openings','entrance_openings','visible_entrance_openings','navigable_entrance_openings',
        'large_entrance_openings','two_block_openings','penetrating_openings']}
    counts['deep_entrance_openings']=sum(m['deep'] for m in mouths)
    counts['visible_deep_entrance_openings']=sum(m['deep'] and m['visible'] is True for m in mouths)
    return dict(windows=len(rows),seeds=len(set(r['seed'] for r in rows)),area_km2=area,
        counts=counts,per_km2={k:v/area for k,v in counts.items()},
        terrain_entrances=dict(sum((collections.Counter(r['terrain_entrances']) for r in rows),collections.Counter())),
        source_entrances=dict(sum((collections.Counter(r['source_entrances']) for r in rows),collections.Counter())),
        sensitivity={str(t):sum(r['entrance_threshold_sensitivity'][str(t)] for r in rows) for t in [2,3,4]},
        horizontal_span=distribution([m['horizontal_span'] for m in mouths]),
        vertical_span=distribution([m['vertical_span'] for m in mouths]),
        exposed_faces=distribution([m['faces'] for m in mouths]),
        face_bins=np.histogram([m['faces'] for m in mouths],[0,4,9,25,64,256,np.inf])[0].tolist(),
        systems=len(groups),systems_with_multiple_entrances=sum(n>1 for n in groups),
        entrances_per_system=distribution(groups),nearest_centroid_distance=distribution(nearest),
        windows_without_entrances=sum(r['entrance_openings']==0 for r in rows))


def bootstrap(old,new,key):
    seeds=sorted(set(r['seed'] for r in old))
    a=np.array([sum(r[key] for r in old if r['seed']==s) for s in seeds])
    b=np.array([sum(r[key] for r in new if r['seed']==s) for s in seeds])
    indices=np.random.default_rng(42).integers(0,len(seeds),(4096,len(seeds)))
    base=a[indices].sum(axis=1);candidate=b[indices].sum(axis=1)
    ratios=1-candidate[base>0]/base[base>0]
    return dict(reduction=1-b.sum()/a.sum() if a.sum() else None,
        seed_bootstrap_95=[float(x) for x in np.percentile(ratios,[2.5,97.5])] if len(ratios) else None,seeds=len(seeds),resamples=4096)


def geometry(rows):
    result={key:sum(r[key] for r in rows) for key in ['deep_geology_cells','deep_changed_cells','new_geological_air','changed_geology_cells']}
    result.update(windows=len(rows),deep_hashes_equal=all(r['deep_before_sha256']==r['deep_after_sha256'] for r in rows),
        changed_distance_bins=np.sum([r['changed_distance_bins'] for r in rows],axis=0).tolist(),
        changed_cover_bins=np.sum([r['changed_cover_bins'] for r in rows],axis=0).tolist())
    for version in ['v15','v16','deep_v15','deep_v16']:
        result[version]={k:sum(r[version][k] for r in rows) for k in ['cavity_cells','components','supported_floor_cells','floor_components']}
        result[version].update({k:distribution([r[version][k] for r in rows]) for k in ['largest_cavity','largest_floor','supported_route']})
        result[version].update({k:np.sum([r[version][k] for r in rows],axis=0).tolist() for k in ['component_size_bins','width_bins','height_bins']})
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--work',required=True,type=Path)
    parser.add_argument('--output',required=True,type=Path)
    args=parser.parse_args();result={}
    for name in ['primary','additional','stress','confirmation','independent']:
        versions={v:read(args.work/f'{v}-{name}.jsonl') for v in ['v15','v16']}
        if (args.work/f'horizon-{name}.jsonl').exists():versions['horizon']=read(args.work/f'horizon-{name}.jsonl')
        cohort={}
        for label,select in [('all',lambda r:True),('unconditional',lambda r:'unconditional' in r['stratum'])]:
            selected={v:[r for r in rows if select(r)] for v,rows in versions.items()}
            cohort[label]={v:surface(rows) for v,rows in selected.items()}
            cohort[label]['comparison']={k:bootstrap(selected['v15'],selected['v16'],k) for k in ['entrance_openings','visible_entrance_openings','penetrating_openings']}
        cohort['strata']={k:{v:surface([r for r in rows if r['stratum']==k]) for v,rows in versions.items()} for k in sorted(set(r['stratum'] for r in versions['v15']))}
        cohort['geometry']=geometry(read(args.work/f'paired-{name}.jsonl'))
        # Keep the previous portal definition for continuity, not as physical-mouth truth.
        legacy={}
        for v,rows in versions.items():
            if rows and 'legacy' in rows[0]:legacy[v]=[dict(r['legacy'],id=r['id'],stratum=r['stratum']) for r in rows]
            else:
                path=Path('docs/performance/slice42/raw')/f'{name}-{v}.jsonl.gz'
                if path.exists():legacy[v]=[json.loads(l) for l in gzip.decompress(path.read_bytes()).splitlines()]
        reference=Path('docs/performance/slice42/raw')/f'{name}-horizon.jsonl.gz'
        if 'horizon' not in legacy and reference.exists():
            legacy['horizon']=[json.loads(l) for l in gzip.decompress(reference.read_bytes()).splitlines()]
        cohort['legacy_portals']={label:{v:sum(r['entrance_portal_clusters'] for r in rows if label=='all' or 'unconditional' in r['stratum'])/(sum(r['n']**2 for r in rows if label=='all' or 'unconditional' in r['stratum'])/1e6) for v,rows in legacy.items()} for label in ['all','unconditional']}
        result[name]=cohort
    result['pooled_geometry']=geometry([r for name in result for r in read(args.work/f'paired-{name}.jsonl')])
    args.output.write_text(json.dumps(result,indent=2)+'\n')

if __name__=='__main__':
    main()
