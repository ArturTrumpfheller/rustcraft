#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Capture genuine v9 vegetation using the read-only RustCraft terrain gallery.

All camera vectors are explicit world coordinates. FOV78, daylight6000, original
production atlas/cutout/terrain/water materials. Requires a working desktop or
Xvfb/Mesa. Software Vulkan is development evidence, not native Radeon performance.
"""
import argparse
import subprocess
from pathlib import Path
from generate_textures import png

# name, seed, center X/Z, world eye, direction, rendered radius, center biome
SCENES=[
 ('plains-flowers',0,(-2432,-4096),(-2424,9,-4082),(-6,-3,-10),3,'Plains'),
 ('mixed-forest',2026,(-896,-4096),(-878,13,-4068),(-12,-7,-22),3,'Forest'),
 ('birch-close',42,(-1591,-4095),(-1584,7,-4088),(-7,0,-7),2,'Plains'),
 ('spruce-close',42,(-755,-4023),(-745,8,-4013),(-10,0,-10),2,'Snowy Plains'),
 ('snow-spruce',0,(-4096,-4096),(-4077,10,-4078),(-13,-3,-16),3,'Snowy Plains'),
 ('desert-shrubs',2026,(3456,-3968),(3463,8,-3949),(0,-3,-15),3,'Desert'),
 ('lake-vegetation',42,(-434,31),(-397,16,62),(-32,-12,-26),4,'Forest'),
 ('density-transition',0,(-2176,-4096),(-2190,17,-4050),(20,-10,-32),4,'Forest'),
]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--executable',type=Path,default=Path('target/debug/examples/terrain_gallery'))
    parser.add_argument('--scene',choices=[s[0] for s in SCENES])
    args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
    for name,seed,center,eye,direction,radius,biome in SCENES:
        if args.scene and args.scene!=name:continue
        ppm=args.output/f'{name}.ppm'
        command=[str(args.executable.resolve()),str(seed),*map(str,center),'0','--generator=9',
                 '--eye='+','.join(map(str,eye)),'--direction='+','.join(map(str,direction)),
                 f'--radius={radius}','--capture-frames=60','--time=6000','--survey-fog',f'--capture={ppm.resolve()}']
        result=subprocess.run(command,capture_output=True,text=True,timeout=300)
        (args.output/f'{name}.log').write_text(' '.join(command)+'\n'+result.stdout+result.stderr)
        result.check_returncode()
        magic,size,maximum,rgb=ppm.read_bytes().split(b'\n',3);width,height=map(int,size.split())
        assert magic==b'P6' and maximum==b'255' and len(rgb)==width*height*3 and len(set(rgb))>16
        rgba=b''.join(rgb[i:i+3]+b'\xff' for i in range(0,len(rgb),3))
        target=args.output/f'{name}.png';target.write_bytes(png(width,height,rgba,compression=9));ppm.unlink()
        print(f'{target}: generator9 seed={seed} center={center} eye={eye} direction={direction} radius={radius} biome={biome}',flush=True)
if __name__=='__main__':main()
