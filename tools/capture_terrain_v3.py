#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Actual read-only Bevy gallery captures. Requires X11 and a wgpu adapter.
Panoramas explicitly use the existing capture-only radius10/survey-fog controls;
the normal scenes use supported radius4 and production fog. No save access.
"""
import argparse,json,os,subprocess,sys,tempfile
from pathlib import Path
from generate_textures import png
SCENES=[
 ('plains',42,-144,752,75,4,None,None),
 ('forest',0,-2176,-4096,-30,4,None,None),
 ('mountain',42,-1004,-3488,90,4,[-1004,56,-3488],[1,.4,0]),
 ('valley',42,-3008,720,-90,10,[-2870,43,720],[-1,.25,0]),
 ('transition',42,-992,-3488,90,10,[-1060,100,-3430],[1,-.2,-.3]),
 ('range',0,-3984,-3152,80,10,[-4060,168,-3070],[1,-.3,-.5]),
 ('coast',42,1376,-592,180,4,[1376,14,-592],[0,-.35,1]),
]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--executable',type=Path,default=Path('target/debug/examples/terrain_gallery'));p.add_argument('--scene',choices=[s[0] for s in SCENES]);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
 records=json.loads((a.output/'captures.json').read_text()) if a.scene and (a.output/'captures.json').exists() else []
 for name,seed,x,z,yaw,radius,eye,direction in SCENES:
  if a.scene and a.scene!=name:continue
  for version in ([9,10] if name in ['mountain','plains','transition'] else [10]):
   dest=a.output/f'{name}-v{version}.png'
   with tempfile.TemporaryDirectory(prefix='rustcraft-s35-capture-') as tmp:
    ppm=Path(tmp)/'capture.ppm';cmd=[str(a.executable.resolve()),str(seed),str(x),str(z),str(yaw),f'--generator={version}',f'--radius={radius}','--capture-frames=45',f'--capture={ppm}']
    if radius>4:cmd.append('--wide-survey-fog')
    if eye is not None:
     # For a meaningful baseline panorama keep the eye above its own landscape.
     pose=eye if version==10 else [eye[0],(11 if name=='mountain' else 32),eye[2]]
     cmd.extend(['--eye='+','.join(map(str,pose)),'--direction='+','.join(map(str,direction))])
    done=subprocess.run(cmd,capture_output=True,text=True,check=True,timeout=240)
    log=done.stdout+done.stderr;(a.output/f'{name}-v{version}.log').write_text(log)
    _,size,_,rgb=ppm.read_bytes().split(b'\n',3);w,h=map(int,size.split());assert len(rgb)==w*h*3
    dest.write_bytes(png(w,h,b''.join(rgb[i:i+3]+b'\xff' for i in range(0,len(rgb),3)),compression=9))
    records=[r for r in records if r['file']!=dest.name]
    records.append({'file':dest.name,'command':[v for v in cmd if not v.startswith('--capture=')],'seed':seed,'generator':version,'center':[x,z],'radius':radius,'capture_only_survey_fog':radius>4,'camera_log':[l for l in log.splitlines() if 'CAMERA' in l or 'SCENE' in l or 'AdapterInfo' in l]})
    print(dest,flush=True)
 (a.output/'captures.json').write_text(json.dumps(records,indent=2)+'\n')
if __name__=='__main__':main()
