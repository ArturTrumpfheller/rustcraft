#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Geology-aware cave integration across different terrain, on recorded windows.

Unlike Slice42's same-geology suppression test, v17 changes cover. Report separate
domains and a joint covered mask, without claiming identical deep cave occupancy.
"""
import argparse
import csv
import json
from pathlib import Path
import numpy as np
from scipy import ndimage
from analyze_cave_volumes import load, graph, routes


def geometry(cells, base, surface, low):
    y=np.arange(cells.shape[2])[None,None,:]+low
    depth=surface[:,:,None]-y
    geology=(base==0)&(depth>=8)&(y>=low+4)
    cavity=geology&np.isin(cells,[1,2,3])
    labels,n=ndimage.label(cavity)
    sizes=np.bincount(labels.ravel())[1:]
    dry=cells==1
    standing=dry&geology
    standing[:,:,0]=standing[:,:,-1]=False
    standing[:,:,1:]&=cells[:,:,:-1]==0
    standing[:,:,:-1]&=dry[:,:,1:]
    network,_=graph(standing,dry)
    _,counts,route=routes(network)
    return dict(geology=int(geology.sum()),cavity=int(cavity.sum()),cavity_fraction=float(cavity.sum()/max(1,geology.sum())),components=int(n),largest_component=int(sizes.max(initial=0)),large_components_ge32768=int((sizes>=32768).sum()),standing_nodes=int(standing.sum()),largest_standing=int(counts.max(initial=0)),supported_route_lower_bound=route)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for key in ['plan','v16','v17','base16','base17','output']:p.add_argument('--'+key,type=Path,required=True)
    a=p.parse_args();a.output.parent.mkdir(parents=True,exist_ok=True)
    with a.output.open('w') as f:
        for row in csv.DictReader(a.plan.open()):
            name=row['id']+'.rcv'
            old,_,meta=load(a.v16/name);new,_,nmeta=load(a.v17/name)
            b16,h16,_=load(a.base16/name);b17,h17,_=load(a.base17/name)
            assert old.shape==new.shape==b16.shape==b17.shape
            y=np.arange(old.shape[2])[None,None,:]+meta['min_y']
            joint=(b16==0)&(b17==0)&(h16[:,:,None]-y>=32)&(h17[:,:,None]-y>=32)&(y>=meta['min_y']+4)&(y<=-16)
            joint[:24]=joint[-24:]=False;joint[:,:24]=joint[:,-24:]=False
            a16=np.isin(old,[1,2,3]);a17=np.isin(new,[1,2,3])
            record=dict(id=row['id'],seed=row['seed'],stratum=row['stratum'],v16=geometry(old,b16,h16,meta['min_y']),v17=geometry(new,b17,h17,nmeta['min_y']),joint_covered_cells=int(joint.sum()),joint_changed_cavity_cells=int(((a16!=a17)&joint).sum()),joint_cavity_v16=int((a16&joint).sum()),joint_cavity_v17=int((a17&joint).sum()))
            f.write(json.dumps(record,separators=(',',':'))+'\n');f.flush();print(row['id'],flush=True)


if __name__=='__main__':main()
