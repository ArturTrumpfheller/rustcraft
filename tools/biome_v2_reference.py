#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Independent integer v12 climate fixtures; reuses the independent v11 reference.
No Rust output is read. CI checks both the Python result and Rust consumption.
"""
import argparse,csv,io
from pathlib import Path
from terrain_v4_reference import hash_value,parameters as terrain,div,MASK
Q=1024
def noise(seed,x,z,scale,salt):
 cx,rx=divmod(x,scale);cz,rz=divmod(z,scale)
 def fade(v):
  t=v*Q//scale;return div(div(t*t,Q)*(3*Q-2*t),Q)
 def at(dx,dz):return hash_value(seed,cx+dx,cz+dz,salt)%2049-Q
 def mix(a,b,t):return a+div((b-a)*t,Q)
 return mix(mix(at(0,0),at(1,0),fade(rx)),mix(at(0,1),at(1,1),fade(rx)),fade(rz))
def climate(seed,x,z):
 wx=x+div(noise(seed,x,z,640,0xc001)*64,Q);wz=z+div(noise(seed,x,z,640,0xc002)*64,Q)
 px,pz=div(wx*4+wz*3,5),div(wz*4-wx*3,5)
 t=div(noise(seed,px,pz,1536,0xc003)*5+noise(seed,px,pz,640,0xc004),6)
 h=div(noise(seed,px,pz,1024,0xc005)*4+noise(seed,px,pz,448,0xc006),5)
 c,e,w=[div(v,4) for v in terrain(seed,x,z)[:3]]
 line=76+div(t*90,Q)+div(max(0,min(Q,-c))*5,Q)+div(e,160)-div(w,160)
 cx=x//32*32;cz=z//32*32;fx=x%32*Q//32;fz=z%32*Q//32
 def at(x,z):return div(terrain(seed,x,z)[3],4)
 def mix(a,b,t):return a+div((b-a)*t,Q)
 elevation=mix(mix(at(cx,cz),at(cx+32,cz),fx),mix(at(cx,cz+32),at(cx+32,cz+32),fx),fz)
 return t,h,c,e,w,line,line-18,elevation

def fixture():
 out=io.StringIO();w=csv.writer(out,lineterminator='\n');w.writerow(['seed','x','z','temperature','humidity','continentalness','erosion','weirdness','snowline','treeline','elevation_q10'])
 for seed in [0,42,2026,MASK]:
  for x,z in [(0,10),(-1,-1),(15,16),(-17,-16),(63,64),(-65,-64),(1023,-1024),(-24000,20000),(28000,-30000),(-32768,32767),(32767,-32768),(-1920,-4032),(3328,-3264),(3520,1728)]:w.writerow([seed,x,z,*climate(seed,x,z)])
 return out.getvalue()
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');a=p.parse_args();path=Path(__file__).resolve().parents[1]/'crates/voxel_core/tests/fixtures/climate-v12.csv'
 if a.check:assert path.read_text()==fixture(),'v12 climate fixture differs'
 else:path.write_text(fixture())
 print('56 independent v12 climate fixtures verified' if a.check else path)
