#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Capture unmodified v7 cave geometry with production RustCraft shaders.

The optional torch fixture places supported floor torches, never Stone/Air.
Camera eye and direction are absolute world XYZ and a direction vector.
"""
import argparse
from pathlib import Path
import subprocess
import tempfile
from generate_textures import png

SCENES = [
    ('small-tunnel',42,-63,-143,(-63,-24,-142),(0.3,0,-1),True),
    ('branching-passage',42,-65,-133,(-65,-30,-127),(0,0.1,-1),True),
    ('medium-chamber',0,-63,-123,(-60,-30,-117),(-0.3,0.08,-1),True),
    ('large-cavern',42,51,-66,(58,-34,-53),(-0.3,0.08,-1),True),
    ('vertical-connection',42,-175,-158,(-175,-39,-158),(-0.7,-0.25,0.7),True),
    ('surface-entrance',2026,31,-39,(30,6,-37),(0.35,-0.6,-1),False),
]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--executable',type=Path,default=Path('target/debug/examples/terrain_gallery'))
    parser.add_argument('--scene',choices=[s[0] for s in SCENES])
    args=parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=True)
    for name,seed,x,z,eye,direction,torches in SCENES:
        if args.scene and args.scene!=name:continue
        destination=args.output/f'{name}.png'
        if destination.exists():parser.error(f'refusing to overwrite {destination}')
        with tempfile.TemporaryDirectory(prefix='rustcraft-caves-') as temporary:
            ppm=Path(temporary)/'capture.ppm'
            command=[str(args.executable.resolve()),str(seed),str(x),str(z),'0','--generator=7',f'--capture={ppm}', '--eye='+','.join(map(str,eye)), '--direction='+','.join(map(str,direction))]
            if torches:command.append('--cave-torches')
            result=subprocess.run(command,check=False,capture_output=True,text=True,timeout=180)
            (args.output/f'{name}.txt').write_text(result.stdout+result.stderr,encoding='utf-8')
            result.check_returncode()
            magic,size,maximum,rgb=ppm.read_bytes().split(b'\n',3)
            assert magic==b'P6' and maximum==b'255'
            width,height=map(int,size.split())
            assert len(rgb)==width*height*3 and len(set(rgb))>16
            rgba=b''.join(rgb[i:i+3]+b'\xff' for i in range(0,len(rgb),3))
            destination.write_bytes(png(width,height,rgba,compression=9))
            print(f'{name}: seed={seed} eye={eye} direction={direction} torch_fixture={torches}',flush=True)

if __name__=='__main__':main()
