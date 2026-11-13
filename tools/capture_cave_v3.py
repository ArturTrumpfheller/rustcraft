#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Genuine Bevy cavity captures. Geometry untouched; illumination explicitly labeled.
The torch-only variant uses supported ordinary torches and no visibility floor.
"""
import argparse,json,subprocess,tempfile
from pathlib import Path
from generate_textures import png
SCENES=[
 ('rich-complex',18446744073709551615,(-181.5,-12.38,-180.5),(0.5,0.05,0.8660254)),
 ('large-cavern',42,(42.5,-25.38,70.5),(0.85,0.10,0.5)),
 ('ordinary-chamber',42,(-36.5,-47.38,76.5),(1,0.06,0.2)),
 ('winding-tunnel',42,(-250.5,-38.38,199.5),(1,0.05,-0.15)),
 ('narrow-passage',0,(-110.5,-21.38,202.5),(0.8,0.05,-0.6)),
 ('connected-levels',42,(-192.5,-26.38,89.5),(0.4,0.2,-1)),
 ('vertical-shaft',2026,(-175.5,-12.38,-83.5),(0.45,0.65,-0.5)),
 ('large-intersection',42,(-180.5,-12.38,72.5),(-0.5,0.05,0.8660254)),
 ('mountain-cave',42,(-719.5,67.62,-564.5),(0.5,0.05,0.8)),
 ('surface-entrance',2026,(94.5,47,-180.5),(-1,-0.35,0.35)),
 ('deep-foundation',42,(-613.5,-50.38,-595.5),(1,0.08,0.1)),
 ('historical-v13-cavern',42,(58,-34,-53),(-0.3,0.08,-1)),
]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',required=True,type=Path);p.add_argument('--executable',type=Path,default=Path('target/release/examples/terrain_gallery'));p.add_argument('--scene',choices=[s[0] for s in SCENES]);p.add_argument('--torch-only',action='store_true');p.add_argument('--skip-existing',action='store_true');a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
 for name,seed,eye,direction in SCENES:
  if a.scene and name!=a.scene:continue
  version=13 if name.startswith('historical') else 14
  label=name+('-torches' if a.torch_only else '')
  dest=a.output/f'{label}.png'
  if a.skip_existing and dest.exists():continue
  assert not dest.exists(),f'refusing overwrite: {dest}'
  with tempfile.TemporaryDirectory(prefix='rustcraft-cave-v3-') as tmp:
   ppm=Path(tmp)/'capture.ppm'
   cmd=[str(a.executable.resolve()),str(seed),str(int(eye[0])),str(int(eye[2])),'0',f'--generator={version}',f'--capture={ppm}','--capture-frames=120','--radius=4','--eye='+','.join(map(str,eye)),'--direction='+','.join(map(str,direction)),'--cave-torches']
   if not a.torch_only:cmd.append('--diagnostic-light')
   result=subprocess.run(cmd,capture_output=True,text=True,timeout=240)
   (a.output/f'{label}.log').write_text(result.stdout+result.stderr)
   result.check_returncode()
   magic,size,maximum,rgb=ppm.read_bytes().split(b'\n',3);assert magic==b'P6' and maximum==b'255'
   w,h=map(int,size.split());assert len(rgb)==w*h*3 and len(set(rgb))>16, f"{label}: colors={len(set(rgb))}, first={rgb[:12]}"
   dest.write_bytes(png(w,h,b''.join(rgb[i:i+3]+b'\xff' for i in range(0,len(rgb),3)),compression=9))
   (a.output/f'{label}.json').write_text(json.dumps(dict(seed=seed,generator=version,eye=eye,direction=direction,diagnostic_light=not a.torch_only,supported_floor_torches=True,command=cmd),indent=2)+'\n')
   print(label,flush=True)
if __name__=='__main__':main()
