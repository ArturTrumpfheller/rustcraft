#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Matched genuine Bevy v10/v11 captures; no saves or synthetic terrain images.
Ground views use supported radius4 and production fog. Explicit panoramas use
only the existing development gallery radius10 and survey fog, never game settings.
"""
import argparse,hashlib,json,subprocess,tempfile
from terrain_v3_reference import height as v10_height
from terrain_v4_reference import parameters, profile
from pathlib import Path
from generate_textures import png
SCENES=[
 ('v10-plains-site',42,-144,752,75,4,None,None),
 ('plains',2026,-2560,-4096,45,4,None,None),
 ('rolling-hills',42,-2880,720,90,4,None,None),
 ('forest',18446744073709551615,-3984,-3152,-30,4,None,None),
 ('foothills',42,-992,-3488,90,4,None,None),
 ('rocky-peaks',0,-3984,-3152,80,4,None,None),
 ('valley',0,-512,-3200,90,4,None,None),
 ('coast-lowlands',0,-2176,-4096,100,4,None,None),
 ('range-panorama',0,-3984,-3152,80,10,[-4094,190,-3062],[1,-.6,-.7]),
 ('peaks-panorama',0,-3360,-2576,80,10,[-3470,190,-2486],[1,-.6,-.7]),
 ('valley-panorama',0,-512,-3200,90,10,[-610,110,-3130],[1,-.5,-.6]),
 ('transition-panorama',42,-992,-3488,90,10,[-1102,150,-3398],[1,-.65,-.7]),
]
def open_yaw(seed,x,z,version):
 # Find a reproducible open ground view instead of pointing into a nearby wall.
 # These independent reference heights select only the CAMERA, never geometry.
 def height(x,z):
  if version==10:return v10_height(seed,x,z)
  p=parameters(seed,x,z);return profile(seed,x,z,p[3],p[4])[0]
 h=height(x,z)
 directions=[(0,-1,0),(1,-1,45),(1,0,90),(1,1,135),(0,1,180),(-1,1,225),(-1,0,270),(-1,-1,315)]
 def score(d):
  dx,dz,_=d
  return max(0,height(x+dx*8,z+dz*8)-h)*20+sum(height(x+dx*r,z+dz*r) for r in [16,32,48])
 return min(directions,key=score)[2]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--executable',type=Path,default=Path('target/debug/examples/terrain_gallery'));p.add_argument('--scene',choices=[s[0] for s in SCENES]);p.add_argument('--resume',action='store_true');a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
 records=json.loads((a.output/'captures.json').read_text()) if (a.output/'captures.json').exists() else []
 for name,seed,x,z,yaw,radius,eye,direction in SCENES:
  if a.scene and a.scene!=name:continue
  for version in [10,11]:
   dest=a.output/f'{name}-v{version}.png'
   pose_yaw=open_yaw(seed,x,z,version) if radius<=4 else yaw
   if a.resume and dest.exists():continue
   with tempfile.TemporaryDirectory(prefix='rustcraft-s36-capture-') as tmp:
    ppm=Path(tmp)/'capture.ppm';cmd=[str(a.executable.resolve()),str(seed),str(x),str(z),str(pose_yaw),f'--generator={version}',f'--radius={radius}','--capture-frames=60',f'--capture={ppm}']
    if radius>4:cmd.append('--wide-survey-fog')
    if eye is not None:cmd.extend(['--eye='+','.join(map(str,eye)),'--direction='+','.join(map(str,direction))])
    done=subprocess.run(cmd,capture_output=True,text=True,check=True,timeout=360)
    log=done.stdout+done.stderr;(a.output/f'{name}-v{version}.log').write_text(log)
    magic,size,maximum,rgb=ppm.read_bytes().split(b'\n',3);w,h=map(int,size.split());assert magic==b'P6' and maximum==b'255' and len(rgb)==w*h*3
    assert len(set(zip(rgb[0::3],rgb[1::3],rgb[2::3])))>64, f'Empty/not-ready renderer capture: {name} v{version}'
    dest.write_bytes(png(w,h,b''.join(rgb[i:i+3]+b'\xff' for i in range(0,len(rgb),3)),compression=9))
    records=[r for r in records if r['file']!=dest.name]
    records.append({'file':dest.name,'sha256':hashlib.sha256(dest.read_bytes()).hexdigest(),'executable_sha256':hashlib.sha256(a.executable.read_bytes()).hexdigest(),'command':[v for v in cmd if not v.startswith('--capture=')],'seed':seed,'generator':version,'center':[x,z],'radius':radius,'development_panorama':radius>4,'camera_log':[l for l in log.splitlines() if 'CAMERA' in l or 'SCENE' in l or 'AdapterInfo' in l]})
    (a.output/'captures.json').write_text(json.dumps(records,indent=2)+'\n');print(dest,flush=True)
if __name__=='__main__':main()
