#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Capture original water fixtures through the production mesh and shaders."""
import argparse
from pathlib import Path
import subprocess
import tempfile
from generate_textures import png

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--executable',type=Path,default=Path('target/debug/examples/water_gallery'))
p.add_argument('--output',type=Path,required=True)
a=p.parse_args()
a.output.mkdir(parents=True,exist_ok=True)
for scene,time in [('pool',6000),('flat',6000),('waterfall',6000),('channel',6000),('night',18000)]:
    with tempfile.TemporaryDirectory() as tmp:
        ppm=Path(tmp)/'capture.ppm'
        r=subprocess.run([str(a.executable.resolve()),scene,'2',f'--time={time}',f'--capture={ppm}'],capture_output=True,text=True,timeout=240,check=True)
        if 'ERROR' in r.stderr:
            raise RuntimeError(r.stderr)
        magic,size,maximum,rgb=ppm.read_bytes().split(b'\n',3)
        assert magic==b'P6' and maximum==b'255'
        width,height=map(int,size.split())
        rgba=b''.join(rgb[i:i+3]+b'\xff' for i in range(0,len(rgb),3))
        path=a.output/f'slice22-{scene}.png'
        path.write_bytes(png(width,height,rgba,compression=9))
        print(path,flush=True)
