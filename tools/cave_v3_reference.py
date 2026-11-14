#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Independent scalar oracle against executed Horizon 0de8932 noise samples.

Uses only Python's standard library. No downloaded game files, Horizon runtime
or platform libraries are needed in CI. Fixtures are measured numeric outputs,
not post-design RustCraft snapshots. The old value-field fixtures are archived.
"""
import argparse
import hashlib
import math
from pathlib import Path
import struct
ROOT=Path(__file__).resolve().parents[1]
MASK=(1<<64)-1
PARAMETERS=[
 ('cave_cheese',-8,.8361300524356068,[.5,1,2,1,2,1,0,2,0]),
 ('cave_layer',-8,.955388882960065,[1]),
 ('cave_entrance',-7,.8500634887071167,[.4,.5,1]),
 *[(n,b,.955388882960065,[1]) for n,b in [
 ('spaghetti_3d_1',-7),('spaghetti_3d_2',-7),('spaghetti_3d_rarity',-11),
 ('spaghetti_3d_thickness',-8),('spaghetti_roughness',-5),('spaghetti_roughness_modulator',-8),
 ('spaghetti_2d',-7),('spaghetti_2d_elevation',-8),('spaghetti_2d_modulator',-11),('spaghetti_2d_thickness',-11)]],
 ('pillar',-7,.9494731054427981,[1,1]),
 *[(n,-8,.955388882960065,[1]) for n in ['pillar_rareness','pillar_thickness','noodle','noodle_thickness']],
 *[(n,-7,.955388882960065,[1]) for n in ['noodle_ridge_a','noodle_ridge_b']]]
def rotate(v,n):return ((v<<n)|(v>>(64-n)))&MASK
def mix(v):
 v=((v^(v>>30))*0xbf58476d1ce4e5b9)&MASK
 v=((v^(v>>27))*0x94d049bb133111eb)&MASK
 return v^(v>>31)
class Random:
 def __init__(self,state):self.a,self.b=state if any(state) else [0x9e3779b97f4a7c15,0x6a09e667f3bcc909]
 @classmethod
 def world(cls,seed):
  s=seed^0x6a09e667f3bcc909
  return cls([mix(s),mix((s+0x9e3779b97f4a7c15)&MASK)])
 def next(self):
  a,b=self.a,self.b
  value=(rotate((a+b)&MASK,17)+a)&MASK
  c=a^b;self.a=rotate(a,49)^c^((c<<21)&MASK);self.b=rotate(c,28)
  return value
 def fork(self):return self.next(),self.next()
 def bound(self,n):
  threshold=((-n)&0xffffffff)%n
  while True:
   value=(self.next()&0xffffffff)*n
   if value&0xffffffff>=threshold:return value>>32
 def double(self):return (self.next()>>11)/2**53
 def named(self,name):
  h=struct.unpack('>QQ',hashlib.md5(name.encode()).digest())
  return Random([self.a^h[0],self.b^h[1]])
G=[(1,1,0),(-1,1,0),(1,-1,0),(-1,-1,0),(1,0,1),(-1,0,1),(1,0,-1),(-1,0,-1),(0,1,1),(0,-1,1),(0,1,-1),(0,-1,-1),(1,1,0),(0,-1,1),(-1,1,0),(0,-1,-1)]
class Perlin:
 def __init__(self,rng):
  self.offsets=[rng.double()*256 for _ in range(3)];self.table=list(range(256))
  for i in range(256):j=i+rng.bound(256-i);self.table[i],self.table[j]=self.table[j],self.table[i]
 def get(self,p):
  p=[v-math.floor(v/33554432+.5)*33554432+o for v,o in zip(p,self.offsets)]
  c=[math.floor(v) for v in p];f=[v-math.floor(v) for v in p]
  perm=lambda n:self.table[n&255]
  values=[]
  for i in range(8):
   d=[i&1,(i>>1)&1,(i>>2)&1];g=G[perm(perm(perm(c[0]+d[0])+c[1]+d[1])+c[2]+d[2])&15]
   values.append(sum(g[j]*(f[j]-d[j]) for j in range(3)))
  for t in [v*v*v*(v*(v*6-15)+10) for v in f]:values=[a+t*(b-a) for a,b in zip(values[::2],values[1::2])]
  return values[0]
def noise(seed,index):
 name,base,amplitude,modifiers=PARAMETERS[index]
 rng=Random(Random.world(seed).fork()).named('minecraft:'+name)
 factories=[Random(rng.fork()),Random(rng.fork())]
 n=len(modifiers);weights=[2**(n-1)/(2**n-1)*v*2**(-i) for i,v in enumerate(modifiers)]
 factor=sum(weights)/3/(sum((.2702247831245211*w)**2 for w in weights)**.5*2**.5)
 layers=[]
 for i,w in enumerate(weights):
  if w:
   for j,r in enumerate(factories):layers.append((Perlin(r.named('octave_'+str(base+i))),2**(base+i)*(1 if j==0 else 1.0181268882175227),factor*amplitude*w))
 return lambda p:sum(a*layer.get([v*f for v in p]) for layer,f,a in layers)
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true');parser.parse_args()
 previous=None;sample=None;count=0
 for row in (ROOT/'crates/voxel_core/tests/fixtures/horizon-noise.csv').read_text().splitlines()[1:]:
  seed,index,x,y,z,bits=map(int,row.split(','))
  if previous!=(seed,index):sample=noise(seed,index);previous=(seed,index)
  expected=struct.unpack('<f',struct.pack('<I',bits))[0];actual=sample([x,y,z])
  assert abs(actual-expected)<=.000002,(row,actual,expected)
  count+=1
 assert count==720
 print(f'{count} independent samples match executed Horizon noise within 2e-6')
if __name__=='__main__':main()
