#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Independent Q10 3D field oracle; signed division follows Rust truncation.
Does not read Rust source or derive expected values from the production generator.
"""
from pathlib import Path
import argparse
MASK=(1<<64)-1
ROOT=Path(__file__).resolve().parents[1]
def trunc(a,b):return (abs(a)//abs(b))*(-1 if (a<0) != (b<0) else 1)
def hash3(seed,x,y,z,salt):
 seed^=((y&MASK)*0x632be59bd9b4e019)&MASK
 v=seed^((x&MASK)*0x9e3779b97f4a7c15&MASK)^((z&MASK)*0xd1b54a32d192ed03&MASK)^salt
 v=((v^(v>>30))*0xbf58476d1ce4e5b9)&MASK
 v=((v^(v>>27))*0x94d049bb133111eb)&MASK
 return v^(v>>31)
def mix(a,b,t,q):return a+trunc((b-a)*t,q)
def noise(seed,p,scale,salt):
 c=[p[i]//scale[i] for i in range(3)]
 f=[]
 for i in range(3):
  t=(p[i]%scale[i])*1024//scale[i];f.append((t*t//1024)*(3072-2*t)//1024)
 v=[hash3(seed,c[0]+(i&1),c[1]+((i>>1)&1),c[2]+((i>>2)&1),salt)%2049-1024 for i in range(8)]
 return mix(mix(mix(v[0],v[1],f[0],1024),mix(v[2],v[3],f[0],1024),f[1],1024),mix(mix(v[4],v[5],f[0],1024),mix(v[6],v[7],f[0],1024),f[1],1024),f[2],1024)
def sample(seed,p,k):
 scale,salt=([32,20,28],0xe301) if k==0 else ([14,12,16],0xe302)
 return interpolate(seed,p,scale,salt)
def interpolate(seed,p,scale,salt):
 base=[v//4*4 for v in p];f=[v%4 for v in p]
 v=[noise(seed,[base[j]+4*((i>>j)&1) for j in range(3)],scale,salt) for i in range(8)]
 return mix(mix(mix(v[0],v[1],f[0],4),mix(v[2],v[3],f[0],4),f[1],4),mix(mix(v[4],v[5],f[0],4),mix(v[6],v[7],f[0],4),f[1],4),f[2],4)
def region(seed,x,z):
 cx,cz=x//512,z//512
 f=[]
 for v in [x,z]:
  t=(v%512)*1024//512;f.append((t*t//1024)*(3072-2*t)//1024)
 v=[hash3(seed,cx+(i&1),0,cz+((i>>1)&1),0xe40a)%2049-1024 for i in range(4)]
 return mix(mix(v[0],v[1],f[0],1024),mix(v[2],v[3],f[0],1024),f[1],1024)
def network(seed,p):
 a,b,c=[interpolate(seed,p,scale,salt) for scale,salt in [([64,40,64],0xe401),([80,52,80],0xe402),([104,60,104],0xe403)]]
 x,z=p[0]//4*4,p[2]//4*4
 t=[560-trunc(region(seed,x+4*(i&1),z+4*((i>>1)&1))*160,1024) for i in range(4)]
 threshold=mix(mix(t[0],t[1],p[0]%4,4),mix(t[2],t[3],p[0]%4,4),p[2]%4,4)
 width=22+max(0,min(800,c+400))//25
 inside=c-abs(a)//5>threshold or (abs(a)<width and abs(b)<width*5//4)
 return [a,b,c,threshold,int(inside)]
def main():
 a=argparse.ArgumentParser();a.add_argument('--check',action='store_true');args=a.parse_args()
 out='seed,x,y,z,coarse,detail,lattice_coarse,lattice_detail\n'
 for seed in [0,42,2026,MASK]:
  for p in [[0,-60,10],[-1,-1,-1],[15,-17,16],[-129,63,127],[28000,120,-30000],[-32768,-59,32767],[32767,190,-32768],[-253,-33,-129],[64,0,64]]:
   v=[noise(seed,p,[32,20,28],0xe301),noise(seed,p,[14,12,16],0xe302),sample(seed,p,0),sample(seed,p,1)]
   out+=','.join(map(str,[seed,*p,*v]))+'\n'
 path=ROOT/'crates/voxel_core/tests/fixtures/caves-v14-fields.csv'
 if args.check:assert path.read_text()==out,'independent cave field fixtures changed'
 else:path.write_text(out)
 out='seed,x,y,z,a,b,c,threshold,inside\n'
 for seed in [0,42,2026,MASK]:
  for p in [[0,-60,10],[-1,-1,-1],[15,-17,16],[-129,63,127],[28000,120,-30000],[-32768,-59,32767],[32767,190,-32768],[-253,-33,-129],[64,0,64]]:
   out+=','.join(map(str,[seed,*p,*network(seed,p)]))+'\n'
 path=ROOT/'crates/voxel_core/tests/fixtures/caves-v14-network.csv'
 if args.check:assert path.read_text()==out,'independent cave network fixtures changed'
 else:path.write_text(out)
if __name__=='__main__':main()
