#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Independent parameter controls, not rendered worlds or Minecraft statistics.
Compare the frozen model, a uniform-relief control, a shorter-region v13 control,
and selected v13. Runtime has only immutable version13, no tuning switches.
"""
import argparse,inspect,json,types
from pathlib import Path
import numpy as np
import terrain_v4_reference as old
import terrain_v5_reference as new

def variant(module, changes):
    # Execute only project-authored local reference math in an isolated namespace.
    source=inspect.getsource(module.parameters)
    for before,after in changes:
        assert before in source
        source=source.replace(before,after)
    scope=dict(vars(module));exec(source,scope)
    return scope['parameters']

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    a=parser.parse_args()
    uniform=variant(old,[('mul(relief,peaks)','mul(mul(relief,peaks),2662)')])
    short=variant(new,[('n(768,768,0xb004)*5+n(320,320,0xb005)*2+n(128,128,0xb006),8','n(512,512,0xb004)*4+n(224,224,0xb005)*2+n(96,96,0xb006),7')])
    result=[]
    for seed in [0,42,2026,2**64-1]:
        for name,fn in [('v12',old.parameters),('uniform-relief-65-percent',uniform),('v13-short-erosion',short),('v13',new.parameters)]:
            fields=[fn(seed,x,z) for z in range(-4096,4096,128) for x in range(-4096,4096,128)]
            h=np.array([v[3]/4096 for v in fields]).reshape(64,64)
            delta=np.r_[np.abs(np.diff(h,axis=0)).ravel(),np.abs(np.diff(h,axis=1)).ravel()]
            result.append(dict(seed=str(seed),variant=name,samples=len(fields),height_quantiles=dict(zip(['p50','p95','p99','max'],map(float,np.percentile(h,[50,95,99,100])))),difference128_quantiles=dict(zip(['p50','p95','p99','max'],map(float,np.percentile(delta,[50,95,99,100])))),density_envelope_above6_fraction=sum(v[4]>6*4096 for v in fields)/len(fields)))
            print(seed,name,flush=True)
    a.output.write_text(json.dumps(result,indent=2)+'\n')
if __name__=='__main__':main()
