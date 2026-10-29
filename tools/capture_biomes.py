#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Real RustCraft production-material terrain captures; requires a desktop/renderer."""
import argparse
from pathlib import Path
import subprocess
import tempfile
from generate_textures import png

# seed, world X/Z, yaw (0 north,90 east), optional high viewpoint.
SCENES = [
    ('plains',42,-1568,-4096,45,False),
    ('forest',42,-4096,-4096,90,False),
    ('desert',0,2304,-4032,0,False),
    ('snowy-plains',2026,-3808,-4096,90,False),
    ('rocky-highlands',42,3824,-4032,0,False),
    ('plains-forest',42,-2936,-3000,90,True),
    ('plains-desert',2026,1288,-3000,90,True),
]


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--executable',type=Path,default=Path('target/debug/examples/terrain_gallery'))
    parser.add_argument('--scene',choices=[scene[0] for scene in SCENES])
    args=parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=True)
    for name,seed,x,z,yaw,overview in SCENES:
        if args.scene and args.scene != name:continue
        destination=args.output/f'{name}.png'
        if destination.exists():parser.error(f'Refusing to overwrite {destination}')
        with tempfile.TemporaryDirectory(prefix='terrain-capture-') as temporary:
            ppm=Path(temporary)/'capture.ppm'
            command=[str(args.executable.resolve()),str(seed),str(x),str(z),str(yaw),f'--capture={ppm}', '--generator=5']
            if overview:command.append('--overview')
            result=subprocess.run(command,check=True,capture_output=True,text=True,timeout=180)
            magic,size,maximum,rgb=ppm.read_bytes().split(b'\n',3)
            assert magic==b'P6' and maximum==b'255'
            width,height=map(int,size.split())
            assert len(rgb)==width*height*3 and len(set(rgb))>16
            rgba=b''.join(rgb[i:i+3]+b'\xff' for i in range(0,len(rgb),3))
            destination.write_bytes(png(width,height,rgba,compression=9))
            for line in (result.stdout+result.stderr).splitlines():
                if 'TERRAIN_SCENE' in line or 'AdapterInfo' in line:print(line,flush=True)
            print(destination,flush=True)
