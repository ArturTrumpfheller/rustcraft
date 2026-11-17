#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Independent Python integer evaluator of v17; never reads Rust output.

Reuses frozen, independently expressed gradient/density primitives, with its own
value-valued Hermite evaluation. --check verifies committed numerical fixtures.
"""
import argparse
import csv
import io
from pathlib import Path
from terrain_v4_reference import Q, MASK, div, mul, mix, ramp, noise2, profile


def curve(v, knots):
    if v <= knots[0][0]:
        return knots[0][1]
    if v >= knots[-1][0]:
        return knots[-1][1]
    slopes = [div((b[1]-a[1])*Q,b[0]-a[0]) for a,b in zip(knots,knots[1:])]
    tangents = [slopes[0]] + [0 if a*b <= 0 else div(2*a*b,a+b) for a,b in zip(slopes,slopes[1:])] + [slopes[-1]]
    i = next(i for i,p in enumerate(knots) if p[0] >= v)
    (x0,y0),(x1,y1) = knots[i-1:i+1]
    t=div((v-x0)*Q,x1-x0);t2=mul(t,t);t3=mul(t2,t)
    basis=[2*t3-3*t2+Q,-2*t3+3*t2,t3-2*t2+t,t3-t2]
    values=[y0,y1,mul(tangents[i-1],x1-x0),mul(tangents[i],x1-x0)]
    return sum(mul(a,b) for a,b in zip(basis,values))


def router(seed,x,z):
    px,pz=div((4*x+3*z)*Q,5),div((4*z-3*x)*Q,5)
    sx=px+noise2(seed,px,pz,896,896,0x4301)*56
    sz=pz+noise2(seed,px,pz,896,896,0x4302)*56
    n=lambda scale,salt:noise2(seed,sx,sz,scale,scale,salt)
    c=div(n(1536,0x4311)*4+n(640,0x4312)*2+n(256,0x4313),7)
    e=div(n(1024,0x4321)*5+n(384,0x4322)*2+n(160,0x4323),8)
    w=div(n(384,0x4331)*4+n(160,0x4332)*2+n(72,0x4333),5)
    pv=max(-Q,Q-3*abs(abs(w)-2*Q//3))
    hills=curve(pv,[(-Q,0),(-1800,4*Q),(0,15*Q),(2200,27*Q),(Q,34*Q)])
    smooth=curve(pv,[(-Q,8*Q),(-2400,14*Q),(-600,34*Q),(1600,78*Q),(Q,122*Q)])
    rugged=curve(pv,[(-Q,10*Q),(-2400,18*Q),(-600,40*Q),(1600,92*Q),(Q,148*Q)])
    mountain=mix(smooth,rugged,ramp(w,-1800,1800))
    rolling=mul(n(96,0x4341),Q+mul(ramp(-e,-400,1200),2*Q))
    plain=rolling+mul(n(224,0x4342),3*Q)
    eroded=curve(e,[(-Q,mountain),(-2300,mountain),(-1650,mix(hills,mountain,2800)),(-1100,hills),(-500,div(hills,3)+plain),(200,plain),(Q,plain)])
    datum=curve(c,[(-Q,-32*Q),(-1800,-22*Q),(-1150,-9*Q),(-650,5*Q),(0,12*Q),(1600,23*Q),(Q,30*Q)])
    coast=div(hills,4)+plain
    shape=curve(c,[(-Q,0),(-1200,0),(-650,coast),(-200,div(eroded*2,3)),(600,eroded),(Q,eroded)])
    inland=ramp(c,-1300,-250)
    ranges=mul(ramp(-e,650,1800),ramp(pv,-1600,2400))
    jaggedness=mul(ranges,ramp(w,-600,1800))
    offset=datum+shape+mul(inland,mul(n(24,0x4343),mul(jaggedness,2*Q)))
    factor=mix(14*Q,3*Q,ranges)
    amplitude=mul(inland,mul(ranges,div(36*Q*Q,factor)))
    excess=max(0,offset-138*Q)
    offset=offset-excess+div(excess*28*Q,excess+28*Q)
    spawn=ramp(max(abs(x),abs(z-10)),12,76)
    return c,e,w,mix(max(offset,5*Q),offset,spawn),mul(amplitude,spawn),pv,factor,jaggedness


def fixtures():
    out=io.StringIO();writer=csv.writer(out,lineterminator='\n')
    writer.writerow(['seed','x','z','c','e','w','offset','amplitude','pv','factor','jaggedness','top','floor','bits'])
    sites=[(0,10),(-1,-1),(15,16),(-17,-16),(63,64),(-65,-64),(1023,-1024),(-24000,20000),(28000,-30000),(-32768,32767),(32767,-32768),(-512,-3200),(2176,3776),(-640,-832),(-22144,21600),(115,421),(-1440,-4000)]
    for seed in [0,42,2026,MASK,123,777]:
        for x,z in sites:
            p=router(seed,x,z);writer.writerow([seed,x,z,*p,*profile(seed,x,z,p[3],p[4])])
    return out.getvalue()


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');a=p.parse_args()
    path=Path(__file__).resolve().parents[1]/'crates/voxel_core/tests/fixtures/terrain-v17.csv'
    data=fixtures()
    if a.check:
        assert path.read_text()==data,'v17 numerical fixtures differ'
    else:
        path.write_text(data)
    print('102 independent v17 field/factor/density fixtures verified' if a.check else path)
