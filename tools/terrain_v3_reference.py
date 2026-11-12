#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Independent Python integer evaluator for v10 pre-water height golden fixtures.
Does not load Rust output. Emit CSV; --check verifies the committed fixture.
"""
import argparse
from pathlib import Path
Q=4096
MASK=(1<<64)-1
def div(a,b):
    return (abs(a)//abs(b))*(-1 if (a<0)!=(b<0) else 1)
def mul(a,b): return div(a*b,Q)
def smooth(v,lo,hi):
    t=max(0,min(Q,div((v-lo)*Q,hi-lo)))
    return mul(mul(t,t),3*Q-2*t)
def blend(a,b,t): return a+mul(b-a,t)
def hash_at(seed,x,z,salt):
    v=seed^((x*0x9e3779b97f4a7c15)&MASK)^((z*0xd1b54a32d192ed03)&MASK)^salt
    v=((v^(v>>30))*0xbf58476d1ce4e5b9)&MASK
    v=((v^(v>>27))*0x94d049bb133111eb)&MASK
    return v^(v>>31)
def field(seed,x,z,scale,salt):
    size=scale*256;cx=x//size;cz=z//size
    tx=smooth(x%size,0,size);tz=smooth(z%size,0,size)
    def at(dx,dz): return hash_at(seed,cx+dx,cz+dz,salt)%8193-Q
    return blend(blend(at(0,0),at(1,0),tx),blend(at(0,1),at(1,1),tx),tz)
def height(seed,x,z):
    px=x*256;pz=z*256
    wx=px+div(field(seed,px,pz,896,0xa001)*112*256,Q)
    wz=pz+div(field(seed,px,pz,896,0xa002)*112*256,Q)
    f=lambda scale,salt:field(seed,wx,wz,scale,salt)
    continental=f(1536,0xa003);region=f(1024,0xa004);erosion=f(768,0xa005)
    upland=mul(smooth(region,-700,2100),Q-smooth(erosion,0,2900))
    ridge=Q-smooth(abs(field(seed,wx+div(wz,3),wz-div(wx,4),256,0xa006)),0,Q)
    valley=Q-smooth(abs(f(640,0xa008)),160,1550)
    relief=mul(upland,Q-mul(valley,3400))
    plain=8*Q+continental*6+f(384,0xa009)*2
    foothills=mul(smooth(region,-1900,1500),Q-mul(valley,3000))
    mountain=38*Q+ridge*85+f(112,0xa007)*9+f(512,0xa00b)*18
    land=plain+mul(f(144,0xa00a)*12,foothills)+mul(mountain,relief)
    ocean=-22*Q+(continental+Q)*3
    detail=mul(f(32,0xa00c),500+div(relief,4))
    h=(blend(ocean,land,smooth(continental,-2400,-400))+detail)//Q
    return blend(max(h,4)*Q,h*Q,smooth(max(abs(x),abs(z-10)),12,76))//Q

def fixture():
    lines=['seed,x,z,height']
    sites=[(0,10),(-1,-1),(-64,64),(63,-65),(-3984,-3152),(-3664,-2704),(-2880,720),(112,3008),(-1936,176),(-24000,20000),(29232,-28400),(-32768,32767),(32767,-32768)]
    for seed in [0,42,2026,MASK]:
        for x,z in sites:lines.append(f'{seed},{x},{z},{height(seed,x,z)}')
    return '\n'.join(lines)+'\n'
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');a=p.parse_args()
    if a.check:
        path=Path(__file__).resolve().parents[1]/'crates/voxel_core/tests/fixtures/terrain-v10.csv'
        assert path.read_text()==fixture(), 'independent v10 golden fixture differs'
        print('Independent v10 integer fixtures verified')
    else:print(fixture(),end='')
