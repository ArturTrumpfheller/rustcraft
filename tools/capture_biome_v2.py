#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Genuine production Bevy biome renders, supported ground views and labeled
radius10 development panoramas. No game render-distance or save changes.
"""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
from generate_textures import png
from capture_terrain_v4 import open_yaw
SCENES=[
 ('plains',0,-3520,-4032,45,4,None,None),
 ('forest',0,2048,-4032,45,4,None,None),
 ('birch-forest',0,-1920,-4032,45,4,None,None),
 ('taiga',0,1600,-4032,45,4,None,None),
 ('meadow',18446744073709551615,-1840,-3808,135,4,None,[1,-.25,1]),
 ('grove',18446744073709551615,-1616,-4032,45,4,None,None),
 ('snowy-slopes',0,-960,2496,45,4,None,None),
 ('stony-peaks',42,688,-1328,315,4,None,[-1,-.45,-1]),
 ('desert',0,-2688,-3200,45,4,None,None),
 ('beach',42,3328,-3264,90,4,None,None),
 ('snowy-plains',0,384,-2752,45,4,None,None),
 ('river',18446744073709551615,-4032,2112,90,4,[-4032,14,2140],[0,-.25,-1]),
 ('meadow-panorama',18446744073709551615,-1760,-3920,45,10,[-1870,150,-3804],[1,-.65,-1]),
 ('grove-panorama',2026,3520,1728,45,10,[3400,145,1840],[1,-.65,-1]),
 ('warm-peaks-panorama',42,768,-896,45,10,[660,180,-780],[1,-.65,-1]),
 ('coast-panorama',42,3328,-3264,90,10,[3220,100,-3140],[1,-.55,-1]),
]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--executable',type=Path,default=Path('target/release/examples/terrain_gallery'));p.add_argument('--scene',choices=[s[0] for s in SCENES]);p.add_argument('--resume',action='store_true');p.add_argument('--matched',action='store_true',help='Also capture selected v11 comparisons');a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
 records=json.loads((a.output/'captures.json').read_text()) if (a.output/'captures.json').exists() else []
 for name,seed,x,z,yaw,radius,eye,direction in SCENES:
  if a.scene and a.scene!=name:continue
  if radius<=4 and eye is None:yaw=open_yaw(seed,x,z,11)
  versions=[11,12] if a.matched and name in ['birch-forest','grove-panorama','warm-peaks-panorama','coast-panorama'] else [12]
  for version in versions:
   dest=a.output/f'{name}-v{version}.png'
   if a.resume and dest.exists():continue
   with tempfile.TemporaryDirectory(prefix='rustcraft-s37-capture-') as tmp:
    ppm=Path(tmp)/'capture.ppm';cmd=[str(a.executable.resolve()),str(seed),str(x),str(z),str(yaw),f'--generator={version}',f'--radius={radius}','--capture-frames=30',f'--capture={ppm}']
    if radius>4:cmd.append('--wide-survey-fog')
    if eye is not None:cmd.append('--eye='+','.join(map(str,eye)))
    if direction is not None:cmd.append('--direction='+','.join(map(str,direction)))
    done=subprocess.run(cmd,capture_output=True,text=True,check=False,timeout=360);log=done.stdout+done.stderr;(a.output/f'{name}-v{version}.log').write_text(log)
    if done.returncode:raise RuntimeError(log[-5000:])
    expected={'plains':'Plains','forest':'Forest','birch-forest':'Birch Forest','taiga':'Taiga','meadow':'Meadow','grove':'Grove','snowy-slopes':'Snowy Slopes','stony-peaks':'Stony Peaks','desert':'Desert','beach':'Beach','snowy-plains':'Snowy Plains','river':'River'}.get(name)
    if version==12 and expected and f'biome={expected}\n' not in log:raise RuntimeError(f'Camera biome does not match {name}: '+log[:500])
    magic,size,maximum,rgb=ppm.read_bytes().split(b'\n',3);w,h=map(int,size.split());assert magic==b'P6' and maximum==b'255' and len(rgb)==w*h*3
    assert len(set(zip(rgb[0::3],rgb[1::3],rgb[2::3])))>64,'empty renderer capture'
    dest.write_bytes(png(w,h,b''.join(rgb[i:i+3]+b'\xff' for i in range(0,len(rgb),3)),compression=9))
    records=[r for r in records if r['file']!=dest.name]
    records.append(dict(file=dest.name,sha256=hashlib.sha256(dest.read_bytes()).hexdigest(),executable_sha256=hashlib.sha256(a.executable.read_bytes()).hexdigest(),command=[v for v in cmd if not v.startswith('--capture=')],seed=seed,generator=version,center=[x,z],radius=radius,development_panorama=radius>4,camera_log=[l for l in log.splitlines() if any(v in l for v in ['CAMERA','SCENE','AdapterInfo'])]))
    (a.output/'captures.json').write_text(json.dumps(records,indent=2)+'\n');print(dest,flush=True)
if __name__=='__main__':main()
