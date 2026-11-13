#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Genuine matched v12/v13 Bevy captures. Ground radius4 uses production fog;
explicit development panoramas use gallery radius10/wide fog only. No saves.
"""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
from functools import lru_cache
from generate_textures import png
from terrain_v4_reference import parameters as old,profile
from terrain_v5_reference import parameters as new
SCENES=[
 ('broad-plains',0,-3584,-3904,4,None,None),
 ('rolling-hills',42,-2880,720,4,None,None),
 ('ordinary-forest',0,1984,-3904,4,None,None),
 ('moderate-uplands',0,-3008,-640,4,None,None),
 ('foothills',42,-992,-3488,4,None,None),
 ('snowy-peaks',42,-768,-640,4,None,None),
 ('mountain-valley',0,-512,-3200,4,None,None),
 ('coastal-lowlands',0,-2688,-2304,4,None,None),
 ('plains-panorama',0,-3584,-3904,10,[-3694,120,-3788],[1,-.55,-1]),
 ('transition-panorama',42,-992,-3488,10,[-1102,150,-3372],[1,-.65,-1]),
 ('range-panorama',42,-768,-640,10,[-878,185,-524],[1,-.6,-1]),
 ('rocky-panorama',2026,-1536,-3904,10,[-1646,180,-3788],[1,-.6,-1]),
 ('valley-panorama',0,-512,-3200,10,[-622,145,-3084],[1,-.6,-1]),
 ('distant-panorama',2026,-23296,20896,10,[-23406,185,21012],[1,-.65,-1]),
 ('meadow-foothills',0,128,960,4,None,None),
 ('grove-foothills',18446744073709551615,-3392,640,4,None,None),
 ('new-lowlands-panorama',0,-2032,-3872,10,[-2142,140,-3756],[1,-.65,-1]),
 ('moderate-hills-panorama',0,1984,-4032,10,[1874,105,-3916],[1,-.45,-1]),
]
def ground_yaw(seed,x,z):
 # Same direction for matched pairs. Choose a modest forward rise in both worlds.
 @lru_cache(maxsize=128)
 def height(x,z,fn):
  p=fn(seed,x,z);return profile(seed,x,z,p[3],p[4])[0]
 directions=[(0,-1,0),(1,-1,45),(1,0,90),(1,1,135),(0,1,180),(-1,1,225),(-1,0,270),(-1,-1,315)]
 def score(d):
  dx,dz,_=d
  return sum(max(0,height(x+dx*r,z+dz*r,fn)-height(x,z,fn))*(8 if r==8 else 1) for fn in [old,new] for r in [8,16,32,48])
 return min(directions,key=score)[2]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--executable',type=Path,default=Path('target/release/examples/terrain_gallery'));p.add_argument('--scene',choices=[s[0] for s in SCENES]);p.add_argument('--resume',action='store_true');a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
 records=json.loads((a.output/'captures.json').read_text()) if (a.output/'captures.json').exists() else []
 for name,seed,x,z,radius,eye,direction in SCENES:
  if a.scene and a.scene!=name:continue
  if a.resume and all((a.output/f'{name}-v{v}.png').exists() for v in [12,13]):continue
  yaw=ground_yaw(seed,x,z) if radius<=4 else 45
  for version in [12,13]:
   dest=a.output/f'{name}-v{version}.png'
   if a.resume and dest.exists():continue
   with tempfile.TemporaryDirectory(prefix='rustcraft-s38-') as tmp:
    ppm=Path(tmp)/'capture.ppm';cmd=[str(a.executable.resolve()),str(seed),str(x),str(z),str(yaw),f'--generator={version}',f'--radius={radius}','--capture-frames=60',f'--capture={ppm}']
    if radius>4:cmd.append('--wide-survey-fog')
    if eye is not None:cmd.append('--eye='+','.join(map(str,eye)))
    if direction is not None:cmd.append('--direction='+','.join(map(str,direction)))
    done=subprocess.run(cmd,capture_output=True,text=True,check=False,timeout=420);log=done.stdout+done.stderr;(a.output/f'{name}-v{version}.log').write_text(log)
    if done.returncode:raise RuntimeError(log[-5000:])
    magic,size,maximum,rgb=ppm.read_bytes().split(b'\n',3);w,h=map(int,size.split());assert magic==b'P6' and maximum==b'255' and len(rgb)==w*h*3
    if len(set(zip(rgb[0::3],rgb[1::3],rgb[2::3])))<=64:
     # Software pipeline compilation can outlive the first frames under load.
     # Retry the identical scene; never retain an unrendered clear-color image.
     cmd=[v.replace('--capture-frames=60','--capture-frames=240') for v in cmd]
     done=subprocess.run(cmd,capture_output=True,text=True,check=True,timeout=420)
     log=done.stdout+done.stderr;(a.output/f'{name}-v{version}.log').write_text(log)
     magic,size,maximum,rgb=ppm.read_bytes().split(b'\n',3);w,h=map(int,size.split())
     assert magic==b'P6' and maximum==b'255' and len(rgb)==w*h*3
     assert len(set(zip(rgb[0::3],rgb[1::3],rgb[2::3])))>64,'empty renderer capture after retry'
    dest.write_bytes(png(w,h,b''.join(rgb[i:i+3]+b'\xff' for i in range(0,len(rgb),3)),compression=9))
    records=[r for r in records if r['file']!=dest.name]
    records.append(dict(file=dest.name,sha256=hashlib.sha256(dest.read_bytes()).hexdigest(),executable_sha256=hashlib.sha256(a.executable.read_bytes()).hexdigest(),command=[v for v in cmd if not v.startswith('--capture=')],seed=seed,generator=version,center=[x,z],radius=radius,development_panorama=radius>4,camera_log=[l for l in log.splitlines() if any(v in l for v in ['CAMERA','SCENE','AdapterInfo'])]))
    (a.output/'captures.json').write_text(json.dumps(records,indent=2)+'\n');print(dest,flush=True)
if __name__=='__main__':main()
