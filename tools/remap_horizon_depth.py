#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Sensitivity only: resample Horizon RCV1 into RustCraft's vertical domain.

Uses the production adapter's sea/foundation mapping, nearest Y, unchanged X/Z.
This is transformed reference geometry, not a native Horizon generation result.
"""
import argparse,csv,struct
from pathlib import Path
import numpy as np
from analyze_cave_volumes import load
p=argparse.ArgumentParser(description=__doc__);p.add_argument('input',type=Path);p.add_argument('plan',type=Path);p.add_argument('output',type=Path);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False)
y=np.arange(-64,192);reference=np.where(y<=2,-64+(y+64)*127/66,y+61);indices=np.rint(reference).astype(int)+64
for row in csv.DictReader(a.plan.open()):
 cells,surface,meta=load(a.input/(row['id']+'.rcv'));assert meta['height']==384 and meta['min_y']==-64
 cells=np.ascontiguousarray(cells[:,:,indices]);n=meta['n'];surface=np.where(cells==0,y[None,None,:],-64).max(axis=2).astype('<i2')
 data=b'RCV1'+struct.pack('<hhhQii',n,256,-64,int(meta['seed']),meta['x'],meta['z'])+surface.tobytes()+cells.tobytes()
 (a.output/(row['id']+'.rcv')).write_bytes(data)
