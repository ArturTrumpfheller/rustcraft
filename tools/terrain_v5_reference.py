#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Independent Q12 v13 reference; shares frozen noise/density math with v11 Python.
No Rust output is read. Fixtures run on Linux and Windows CI.
"""
import argparse,csv,io
from pathlib import Path
from terrain_v4_reference import Q,MASK,div,mul,mix,clamp,ramp,spline,noise2,profile
def parameters(seed,x,z):
 px,pz=div((4*x+3*z)*Q,5),div((4*z-3*x)*Q,5)
 n=lambda sx,sz,salt:noise2(seed,px,pz,sx,sz,salt)
 c=div(n(1536,1536,0xb001)*4+n(640,640,0xb002)*2+n(256,256,0xb003),7)
 e=div(n(768,768,0xb004)*5+n(320,320,0xb005)*2+n(128,128,0xb006),8)
 a=div(n(112,448,0xb007)*3+n(48,144,0xb008)*2+n(24,56,0xb009),6)
 b=div(n(448,112,0xb017)*3+n(144,48,0xb018)*2+n(56,24,0xb019),6)
 w=mix(a,b,ramp(n(896,896,0xb00f),-900,900))
 inland=ramp(c,-1300,-250)
 datum=spline(c,[(-4096,-32),(-1800,-22),(-1150,-9),(-650,5),(0,14),(1600,23),(4096,30)])
 relief=spline(e,[(-4096,138),(-2400,128),(-1600,108),(-1000,58),(-400,18),(200,7),(900,3),(4096,2)])
 peaks=div(spline(w,[(-4096,12),(-2300,14),(-1300,20),(-450,29),(450,49),(1300,82),(2300,124),(4096,180)]),100)
 jagged=mul(ramp(-e,600,1900),ramp(w,200,1600))
 rolling=mul(n(72,72,0xb00a),Q//3+mul(Q-ramp(e,-300,1200),3*Q));detail=mul(n(18,18,0xb00b),mul(jagged,5*Q))
 offset=datum+mul(inland,mul(relief,peaks)+rolling+detail);excess=max(0,offset-130*Q)
 offset=offset-excess+div(excess*32*Q,excess+32*Q)
 amplitude=mul(inland,mul(mul(ramp(-e,600,1900),ramp(w,-900,1100)),12*Q));spawn=ramp(max(abs(x),abs(z-10)),12,76)
 return c,e,w,mix(max(offset,5*Q),offset,spawn),mul(amplitude,spawn)

def fixtures():
 out=io.StringIO();w=csv.writer(out,lineterminator='\n');w.writerow(['seed','x','z','continentalness','erosion','weirdness','offset','amplitude','top','floor','bits'])
 sites=[(0,10),(-1,-1),(15,16),(-17,-16),(63,64),(-65,-64),(1023,-1024),(-24000,20000),(28000,-30000),(-32768,32767),(32767,-32768),(-512,-3200),(2176,3776),(-640,-832),(-22144,21600),(115,421),(-1440,-4000)]
 for seed in [0,42,2026,MASK]:
  for x,z in sites:
   p=parameters(seed,x,z);w.writerow([seed,x,z,*p,*profile(seed,x,z,p[3],p[4])])
 return out.getvalue()
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');a=p.parse_args();path=Path(__file__).resolve().parents[1]/'crates/voxel_core/tests/fixtures/terrain-v13.csv'
 if a.check:assert path.read_text()==fixtures(),'v13 fixtures differ'
 else:path.write_text(fixtures())
 print('68 independent v13 field/density fixtures verified' if a.check else path)
