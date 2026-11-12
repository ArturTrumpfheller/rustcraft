#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Independent integer reference for v11, including sparse 3D interpolation.
No Rust output is read. Q12 arithmetic truncates toward zero except Euclidean
cell splitting and integer block-height floors. Fixtures are checked in both CI OSes.
"""
import argparse,csv,io
from pathlib import Path
Q=4096
MASK=(1<<64)-1
ROOT=Path(__file__).resolve().parent.parent

def div(a,b): return (abs(a)//abs(b)) * (-1 if (a<0)!=(b<0) else 1)
def mul(a,b):return div(a*b,Q)
def mix(a,b,t):return a+mul(b-a,t)
def clamp(a,lo,hi):return min(hi,max(lo,a))
def hash_value(seed,x,z,salt):
 v=seed ^ ((x*0x9e3779b97f4a7c15)&MASK) ^ ((z*0xd1b54a32d192ed03)&MASK) ^ salt
 v=((v^(v>>30))*0xbf58476d1ce4e5b9)&MASK
 v=((v^(v>>27))*0x94d049bb133111eb)&MASK
 return v^(v>>31)
def fade(t):return mul(mul(mul(t,t),t),mul(t,6*t-15*Q)+10*Q)
def ramp(v,lo,hi):
 t=clamp(div((v-lo)*Q,hi-lo),0,Q);return mul(mul(t,t),3*Q-2*t)
def spline(v,knots):
 if v<=knots[0][0]:return knots[0][1]*Q
 if v>=knots[-1][0]:return knots[-1][1]*Q
 i=next(i for i,p in enumerate(knots) if p[0]>=v)
 slopes=[div((b[1]-a[1])*Q*Q,b[0]-a[0]) for a,b in zip(knots,knots[1:])]
 def tangent(j):
  if j==0:return slopes[0]
  if j==len(knots)-1:return slopes[-1]
  a,b=slopes[j-1:j+1];return 0 if a*b<=0 else div(2*a*b,a+b)
 (x0,y0),(x1,y1)=knots[i-1:i+1];t=div((v-x0)*Q,x1-x0);t2=mul(t,t);t3=mul(t2,t)
 return (2*t3-3*t2+Q)*y0+(-2*t3+3*t2)*y1+mul(t3-2*t2+t,mul(tangent(i-1),x1-x0))+mul(t3-t2,mul(tangent(i),x1-x0))
def noise2(seed,x,z,sx,sz,salt):
 phase=hash_value(seed,0,0,salt^0xb000);px=x+phase%(sx*Q);pz=z+(phase>>32)%(sz*Q)
 cx,fx=divmod(px,sx*Q);cz,fz=divmod(pz,sz*Q);fx//=sx;fz//=sz
 def dot(dx,dz):
  a,b=fx-dx*Q,fz-dz*Q
  return [a,-a,b,-b,div((a+b)*2896,Q),div((a-b)*2896,Q),div((-a+b)*2896,Q),div((-a-b)*2896,Q)][hash_value(seed,cx+dx,cz+dz,salt)&7]
 return 2*mix(mix(dot(0,0),dot(1,0),fade(fx)),mix(dot(0,1),dot(1,1),fade(fx)),fade(fz))
def noise3(seed,p,scale,salt):
 phase=hash_value(seed,0,0,salt^0xb100)
 p=[a*Q+(phase>>(i*17))%(s*Q) for i,(a,s) in enumerate(zip(p,scale))]
 c=[a//(s*Q) for a,s in zip(p,scale)];f=[a%(s*Q)//s for a,s in zip(p,scale)];values=[]
 for i in range(8):
  d=[(i>>k)&1 for k in range(3)];a=[f[k]-d[k]*Q for k in range(3)]
  h=hash_value(seed ^ (((c[1]+d[1])*0x632be59bd9b4e019)&MASK),c[0]+d[0],c[2]+d[2],salt);axis=h%3
  values.append(div(((a[axis] if h&4==0 else -a[axis])+(a[(axis+1)%3] if h&8==0 else -a[(axis+1)%3]))*2896,Q))
 f=list(map(fade,f));v=values
 return 2*mix(mix(mix(v[0],v[1],f[0]),mix(v[2],v[3],f[0]),f[1]),mix(mix(v[4],v[5],f[0]),mix(v[6],v[7],f[0]),f[1]),f[2])
def parameters(seed,x,z):
 px,pz=div((4*x+3*z)*Q,5),div((4*z-3*x)*Q,5)
 n=lambda sx,sz,salt:noise2(seed,px,pz,sx,sz,salt)
 c=div(n(1536,1536,0xb001)*4+n(640,640,0xb002)*2+n(256,256,0xb003),7)
 e=div(n(512,512,0xb004)*4+n(224,224,0xb005)*2+n(96,96,0xb006),7)
 a=div(n(112,448,0xb007)*3+n(48,144,0xb008)*2+n(24,56,0xb009),6)
 b=div(n(448,112,0xb017)*3+n(144,48,0xb018)*2+n(56,24,0xb019),6)
 w=mix(a,b,ramp(n(896,896,0xb00f),-900,900))
 inland=ramp(c,-1300,-250)
 datum=spline(c,[(-4096,-32),(-1800,-22),(-1150,-9),(-650,5),(0,14),(1600,23),(4096,30)])
 relief=spline(e,[(-4096,138),(-2000,128),(-900,105),(-100,54),(650,12),(1600,3),(4096,2)])
 peaks=div(spline(w,[(-4096,0),(-2300,1),(-1300,4),(-450,17),(450,40),(1300,75),(2300,120),(4096,180)]),100)
 jagged=mul(ramp(-e,200,2000),ramp(w,200,1600))
 rolling=mul(n(72,72,0xb00a),Q//3+mul(Q-ramp(e,-300,1200),3*Q));detail=mul(n(18,18,0xb00b),mul(jagged,5*Q))
 offset=datum+mul(inland,mul(relief,peaks)+rolling+detail);excess=max(0,offset-130*Q)
 offset=offset-excess+div(excess*32*Q,excess+32*Q)
 amplitude=mul(inland,mul(ramp(div(relief,Q),16,100),12*Q));spawn=ramp(max(abs(x),abs(z-10)),12,76)
 return c,e,w,mix(max(offset,5*Q),offset,spawn),mul(amplitude,spawn)
def profile(seed,x,z,offset,amplitude):
 if amplitude==0:return offset//Q,offset//Q+1,0
 a=(amplitude+Q-1)//Q;floor=offset//Q-a;end=offset//Q+a+1
 lattice=lambda p:div(noise3(seed,p,[32,28,32],0xb020)*2+noise3(seed,p,[12,8,12],0xb021),3)
 cx,cz=x//4*4,z//4*4;fx,fz=x%4*Q//4,z%4*Q//4;bits=0
 for y in range(floor,end+1):
  cy=y//4*4
  def plane(py):return mix(mix(lattice([cx,py,cz]),lattice([cx+4,py,cz]),fx),mix(lattice([cx,py,cz+4]),lattice([cx+4,py,cz+4]),fx),fz)
  n=clamp(mix(plane(cy),plane(cy+4),y%4*Q//4),-Q,Q)
  if offset-y*Q+mul(amplitude,n)>=0:bits|=1<<(y-floor)
 return floor+bits.bit_length()-1,floor,bits
def fixtures():
 out=io.StringIO();w=csv.writer(out,lineterminator='\n');w.writerow(['seed','x','z','continentalness','erosion','weirdness','offset','amplitude','top','floor','bits'])
 sites=[(0,10),(-1,-1),(15,16),(-17,-16),(63,64),(-65,-64),(1023,-1024),(-24000,20000),(28000,-30000),(-32768,32767),(32767,-32768),(-512,-3200),(2176,3776),(-640,-832),(-22144,21600),(115,421),(-1440,-4000)]
 for seed in [0,42,2026,MASK]:
  for x,z in sites:
   p=parameters(seed,x,z);w.writerow([seed,x,z,*p,*profile(seed,x,z,p[3],p[4])])
 return out.getvalue()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');args=p.parse_args();path=ROOT/'crates/voxel_core/tests/fixtures/terrain-v11.csv';result=fixtures()
 if args.check:assert path.read_text()==result,'v11 reference fixtures differ'
 else:path.write_text(result)
 print('68 independent v11 field/density fixtures verified' if args.check else path)
if __name__=='__main__':main()
