#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Surface apertures against an explicitly cave-disabled geological control.
Exposed geometric mouths, local standing-view visibility, deep cavity connection
and supported navigation are distinct. No noise-field surrogate counts as a hole.
"""
import argparse,csv,hashlib,json
from pathlib import Path
import numpy as np
from scipy import ndimage
from scipy.sparse import csgraph
from analyze_cave_volumes import load,graph,summarize
FAMILIES={'cheese':1,'broad_entrance':2,'spaghetti3':4,'spaghetti2':8,'junction_blend':16,'terrain_adapter':32,'noodle':64,'traditional_cave':128,'canyon':256}

def line_clear(air,start,end):
    """Conservative voxel supercover DDA, including all tied-axis neighbors."""
    start=np.asarray(start,float);end=np.asarray(end,float)
    p=np.floor(start).astype(int);target=np.floor(end).astype(int);d=end-start
    step=np.sign(d).astype(int)
    delta=np.divide(1.,abs(d),out=np.full(3,np.inf),where=d!=0)
    edge=np.where(step>0,p+1,p)
    t=np.divide(edge-start,d,out=np.full(3,np.inf),where=d!=0)
    for _ in range(sum(air.shape)+4):
        if np.any(p<0) or np.any(p>=air.shape) or not air[tuple(p)]:return False
        if np.array_equal(p,target):return True
        hit=np.flatnonzero(abs(t-t.min())<1e-10)
        for bits in range(1,1<<len(hit)):
            q=p.copy()
            for j,axis in enumerate(hit):
                if bits&(1<<j):q[axis]+=step[axis]
            if np.any(q<0) or np.any(q>=air.shape) or not air[tuple(q)]:return False
        p[hit]+=step[hit];t[hit]+=delta[hit]
    return False

def shifted(a,axis,amount):
    b=np.roll(a,amount,axis=axis);cut=[slice(None)]*3;cut[axis]=0 if amount==1 else -1;b[tuple(cut)]=False;return b

def supported_navigation(cells,geology,exterior,depth,y,low):
    dry=cells==1
    standing=dry&(depth>=-1)&(y>=low+4)
    standing[:,:,0]=standing[:,:,-1]=False
    standing[:,:,1:] &= cells[:,:,:-1]==0
    standing[:,:,:-1] &= dry[:,:,1:]
    g,coords=graph(standing,dry)
    eligible=np.zeros(0,bool);floor_labels=np.zeros(0,np.int32)
    near_floor=np.zeros_like(standing)
    if len(coords):
        _,floor_labels=csgraph.connected_components(g,directed=False)
        in_cave=geology[tuple(coords.T)]
        ds=depth[tuple(coords.T)]
        eligible=np.zeros(floor_labels.max()+1,bool)
        eligible[np.unique(floor_labels[in_cave&(ds>=24)])]=True
        deep_counts=np.bincount(floor_labels[in_cave&(ds>=8)],minlength=len(eligible))
        eligible&=deep_counts>=32
        outside=np.zeros_like(eligible)
        outside[np.unique(floor_labels[exterior[tuple(coords.T)]])]=True
        eligible&=outside
        near_floor[tuple(coords[in_cave&eligible[floor_labels]].T)]=True
        near_floor=ndimage.maximum_filter(near_floor,size=3,mode='constant')
    entry_floor=standing&geology&shifted(geology,2,-1)
    near_entry=ndimage.maximum_filter(entry_floor,size=3,mode='constant')
    surface_coords=coords[exterior[tuple(coords.T)]] if len(coords) else np.empty((0,3),int)
    return near_floor,near_entry,surface_coords

def exposure(cells,base,ground,meta,tags=None,previous=None,visibility=True,surface_offset=0):
    assert cells.shape==base.shape and cells.shape[0]==cells.shape[1]
    n,_,height=cells.shape;low=meta['min_y'];y=np.arange(height)[None,None,:]+low
    geology=base==0
    exterior=base==1
    seeds=np.zeros_like(exterior);seeds[:,:,-1]=exterior[:,:,-1]
    for axis in [0,1]:
        for side in [0,-1]:
            cut=[slice(None)]*3;cut[axis]=side;seeds[tuple(cut)]=exterior[tuple(cut)]
    exterior=ndimage.binary_propagation(seeds,mask=exterior)
    void=(cells==1)|(cells==2)|(cells==3)
    cavity=geology&void
    depth=ground[:,:,None]-y
    labels,count=ndimage.label(cavity)
    sizes=np.bincount(labels.ravel());maxdepth=ndimage.maximum(depth,labels,index=np.arange(count+1)) if count else np.array([0])
    faces=np.zeros(cells.shape,np.uint8);roof=np.zeros_like(faces);sidefaces=np.zeros_like(faces)
    for axis in range(3):
        for offset in [-1,1]:
            hit=cavity&shifted(exterior,axis,offset);faces+=hit
            if axis==2 and offset==-1:roof+=hit
            if axis!=2:sidefaces+=hit
    aperture=faces>0
    mouth_labels,mouth_count=ndimage.label(aperture,structure=np.ones((3,3,3),bool))
    dry=cells==1
    near_floor,near_entry,surface_coords=supported_navigation(cells,geology,exterior,depth,y,low)
    slope=np.maximum(abs(np.gradient(ground.astype(float),axis=0)),abs(np.gradient(ground.astype(float),axis=1)))
    relief=ndimage.maximum_filter(ground,size=33)-ndimage.minimum_filter(ground,size=33)
    mean=ndimage.uniform_filter(ground.astype(float),size=33)
    wet=(base==2).any(axis=2);coast=ndimage.distance_transform_edt(~wet)
    distance=ndimage.distance_transform_edt(geology)
    mouths=[];censored=0
    boxes=ndimage.find_objects(mouth_labels)
    for ident,box in enumerate(boxes,1):
        if box is None:continue
        mask=mouth_labels[box]==ident;local=np.array(np.nonzero(mask)).T
        points=local+np.array([b.start for b in box]);center=points.mean(axis=0)
        if any(box[a].start==0 or box[a].stop==n for a in [0,1]):censored+=1;continue
        if not all(8<=center[a]<n-8 for a in [0,1]):continue
        system_ids=np.unique(labels[tuple(points.T)]);system_ids=system_ids[system_ids!=0]
        deep=bool(any(maxdepth[i]>=24 and sizes[i]>=128 for i in system_ids))
        navigable=deep and bool(near_floor[tuple(points.T)].any())
        zz,xx=np.floor(center[:2]).astype(int)
        terrain=('coast' if coast[zz,xx]<=8 else 'cliff' if slope[zz,xx]>=1.5 else 'mountain' if ground[zz,xx]-surface_offset>=78 else 'valley' if ground[zz,xx]<mean[zz,xx]-6 else 'hill' if ground[zz,xx]-surface_offset>=43 or relief[zz,xx]>=12 else 'plain')
        local_box=tuple(slice(max(0,b.start-8),min(cells.shape[i],b.stop+8)) for i,b in enumerate(box))
        local_seed=mouth_labels[local_box]==ident
        reached=ndimage.binary_dilation(local_seed,iterations=8,mask=cavity[local_box])
        penetration=float(distance[local_box][reached].max(initial=0))
        human=bool(near_entry[tuple(points.T)].any())
        visible=None;tested=0
        if visibility and (human or penetration>=3):
            visible=False
            delta=surface_coords[:,:2]-center[:2];dist=(delta*delta).sum(axis=1)
            choices=np.flatnonzero((dist>=1)&(dist<=24**2))
            # Nearby supported samples in each quadrant, not a cave-rich path.
            chosen=[]
            for dz in [-1,1]:
                for dx in [-1,1]:
                    group=choices[(delta[choices,0]*dz>=0)&(delta[choices,1]*dx>=0)]
                    order=group[np.argsort(dist[group],kind='stable')]
                    if len(order):chosen.extend(order[np.linspace(0,min(len(order)-1,127),4).astype(int)].tolist())
            targets=[points[np.argmin(abs(points[:,2]-v))].astype(float)+.5 for v in [np.median(points[:,2]),points[:,2].max(),points[:,2].min()]]
            for i in dict.fromkeys(chosen):
                start=surface_coords[i].astype(float)+[.5,.5,1.62]
                for end in targets:
                    tested+=1
                    if line_clear(dry,start,end):visible=True;break
                if visible:break
        source=0
        if tags is not None:
            for value in np.unique(tags[tuple(points.T)]):source|=int(value)
        spans=[int(b.stop-b.start) for b in box]
        mouths.append(dict(id=ident,box=[[b.start,b.stop] for b in box],center=center.tolist(),
            two_block=human,penetration=penetration,penetrating=penetration>=3,voxels=len(points),faces=int(faces[box][mask].sum()),roof_faces=int(roof[box][mask].sum()),side_faces=int(sidefaces[box][mask].sum()),
            horizontal_span=max(spans[:2]),vertical_span=spans[2],systems=system_ids.tolist(),deep=deep,navigable=navigable,
            visible=visible,visibility_rays=tested,terrain=terrain,source_bits=source,
            isolated_shallow=not deep,large=bool(len(points)>=32 and deep)))
    groups={}
    for mouth in mouths:
        for i in mouth['systems']:groups.setdefault(i,[]).append(mouth['id'])
    deepmask=geology&(distance>24)&(depth>=24)&(y>=low+4)
    deepmask[:24]=deepmask[-24:]=False;deepmask[:,:24]=deepmask[:,-24:]=False
    changed=created=0
    if previous is not None:
        changed=int(((cells!=previous)&deepmask).sum())
        created=int((cavity&~((previous==1)|(previous==2)|(previous==3))).sum())
    included=np.isin(mouth_labels,[m['id'] for m in mouths])
    def counts(key):return {k:sum(m[key]==k for m in mouths) for k in sorted({m[key] for m in mouths})}
    result = dict(metric_version=4,terrain_height_offset=surface_offset,openings=len(mouths),
        two_block_openings=sum(m['two_block'] for m in mouths),
        two_block_visible_openings=sum(m['two_block'] and m['visible'] is True for m in mouths),
        two_block_deep_openings=sum(m['two_block'] and m['deep'] for m in mouths),
        two_block_navigable_openings=sum(m['two_block'] and m['navigable'] for m in mouths),
        source_two_block={key:sum(m['two_block'] and bool(m['source_bits']&bit) for m in mouths) for key,bit in FAMILIES.items()},
        penetrating_openings=sum(m['penetrating'] for m in mouths),
        penetrating_visible_openings=sum(m['penetrating'] and m['visible'] is True for m in mouths),
        penetrating_navigable_openings=sum(m['penetrating'] and m['navigable'] for m in mouths),
        penetrating_deep_openings=sum(m['penetrating'] and m['deep'] for m in mouths),
        penetration_threshold_counts={str(t):sum(m['penetration']>=t for m in mouths) for t in [2,3,4]},
        surface_scars=sum(not m['penetrating'] for m in mouths),
        source_penetrating={key:sum(m['penetrating'] and bool(m['source_bits']&bit) for m in mouths) for key,bit in FAMILIES.items()},
        penetrating_terrain={key:sum(m['penetrating'] and m['terrain']==key for m in mouths) for key in ['coast','cliff','mountain','valley','hill','plain']},visible_openings=sum(m['visible'] is True for m in mouths),
        navigable_openings=sum(m['navigable'] for m in mouths),deep_openings=sum(m['deep'] for m in mouths),
        isolated_shallow_openings=sum(m['isolated_shallow'] for m in mouths),large_openings=sum(m['large'] for m in mouths),
        exposed_systems=len(groups),systems_with_multiple_openings=sum(len(v)>1 for v in groups.values()),
        boundary_censored_openings=censored,opening_area_m2=(n-16)**2,terrain_openings=counts('terrain'),
        source_openings={key:sum(bool(m['source_bits']&bit) for m in mouths) for key,bit in FAMILIES.items()},
        source_aperture_voxels={key:int(((tags&bit)!=0)[included].sum()) if tags is not None else None for key,bit in FAMILIES.items()},
        unclassified_aperture_voxels=int((tags[included]==0).sum()) if tags is not None else None,
        deep_geology_cells=int(deepmask.sum()),deep_cavity_cells=int(cavity[deepmask].sum()),deep_changed_cells=changed,new_cavity_cells=created,
        opening_faces_bins=np.histogram([m['faces'] for m in mouths],[0,4,9,25,64,256,np.inf])[0].tolist(),mouths=mouths)
    return entrance_summary(result)

def entrance_summary(result):
    mouths=result['mouths']
    qualifies=lambda m:m['two_block'] or m['penetration']>=3
    result.update(entrance_openings=sum(qualifies(m) for m in mouths),
        visible_entrance_openings=sum(qualifies(m) and m['visible'] is True for m in mouths),
        navigable_entrance_openings=sum(qualifies(m) and m['navigable'] for m in mouths),
        large_entrance_openings=sum(qualifies(m) and m['deep'] and m['faces']>=64 for m in mouths),
        entrance_threshold_sensitivity={str(t):sum(m['two_block'] or m['penetration']>=t for m in mouths) for t in [2,3,4]},
        source_entrances={key:sum(qualifies(m) and bool(m['source_bits']&bit) for m in mouths) for key,bit in FAMILIES.items()},
        terrain_entrances={k:sum(qualifies(m) and m['terrain']==k for m in mouths) for k in ['coast','cliff','mountain','valley','hill','plain']})
    return result

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('directory',type=Path);p.add_argument('--base',required=True,type=Path);p.add_argument('--plan',required=True,type=Path);p.add_argument('--output',required=True,type=Path);p.add_argument('--previous',type=Path);p.add_argument('--sources',type=Path);p.add_argument('--legacy',action='store_true');p.add_argument('--surface-offset',type=int,default=0,help='Terrain category height offset only; use61 for native Horizon Y');a=p.parse_args()
    a.output.parent.mkdir(parents=True,exist_ok=True)
    with a.output.open('x') as f:
        for row in csv.DictReader(a.plan.open()):
            cells,surface,meta=load(a.directory/(row['id']+'.rcv'));base,ground,bmeta=load(a.base/(row['id']+'.rcv'))
            assert all(meta[k]==bmeta[k] for k in ['seed','x','z','n','height','min_y'])
            previous=load(a.previous/(row['id']+'.rcv'))[0] if a.previous else None
            tags=np.fromfile(a.sources/(row['id']+'.src'),'<u2').reshape(cells.shape) if a.sources else None
            result=exposure(cells,base,ground,meta,tags,previous,surface_offset=a.surface_offset)
            if a.legacy:result['legacy']=summarize(cells,surface,meta)
            result.update(**meta,id=row['id'],stratum=row['stratum'])
            f.write(json.dumps(result,separators=(',',':'))+'\n');f.flush();print(row['id'],result['openings'],result['visible_openings'],result['deep_changed_cells'],flush=True)
if __name__=='__main__':main()
