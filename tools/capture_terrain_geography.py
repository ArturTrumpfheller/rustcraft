#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Execute a recorded matched v16/v17 gallery plan through production Bevy.

Normal views use supported radius4 and ordinary fog. Explicit panoramas use the
development gallery only; neither gameplay radius nor generated terrain changes.
"""
import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path
from generate_textures import png


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--plan',required=True,type=Path)
    p.add_argument('--output',required=True,type=Path)
    p.add_argument('--executable',required=True,type=Path)
    p.add_argument('--scene')
    p.add_argument('--resume',action='store_true')
    a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
    binary=a.executable.resolve();binary_hash=hashlib.sha256(binary.read_bytes()).hexdigest()
    manifest=a.output/'captures.json'
    records=json.loads(manifest.read_text()) if manifest.exists() else []
    for scene in json.loads(a.plan.read_text()):
        if a.scene and scene['name']!=a.scene:continue
        for version in [16,17]:
            dest=a.output/f'{scene["name"]}-v{version}.png'
            if a.resume and dest.exists():continue
            with tempfile.TemporaryDirectory(prefix='rustcraft-s43-') as tmp:
                capture=Path(tmp)/'capture.ppm'
                cmd=[str(binary),str(scene['seed']),str(scene['x']),str(scene['z']),str(scene.get('yaw',45)),f'--generator={version}',f'--radius={scene["radius"]}','--capture-frames=60',f'--capture={capture}']
                if scene['radius']>4:cmd.append('--wide-survey-fog')
                if 'eye' in scene:cmd.append('--eye='+','.join(map(str,scene['eye'])))
                if 'direction' in scene:cmd.append('--direction='+','.join(map(str,scene['direction'])))
                for attempt in range(2):
                    done=subprocess.run(cmd,capture_output=True,text=True,timeout=600)
                    log=done.stdout+done.stderr
                    (a.output/f'{dest.stem}.log').write_text(log)
                    if done.returncode:raise RuntimeError(log[-5000:])
                    magic,size,maximum,rgb=capture.read_bytes().split(b'\n',3)
                    w,h=map(int,size.split());assert magic==b'P6' and maximum==b'255' and len(rgb)==w*h*3
                    if len(set(zip(rgb[0::3],rgb[1::3],rgb[2::3])))>64:break
                    cmd=[v.replace('--capture-frames=60','--capture-frames=240') for v in cmd]
                else:raise RuntimeError('empty/uncompiled renderer capture')
                dest.write_bytes(png(w,h,b''.join(rgb[i:i+3]+b'\xff' for i in range(0,len(rgb),3)),compression=9))
                records=[r for r in records if r['file']!=dest.name]
                records.append(dict(file=dest.name,sha256=hashlib.sha256(dest.read_bytes()).hexdigest(),executable_sha256=binary_hash,scene=scene,generator=version,development_panorama=scene['radius']>4,command=[v for v in cmd if not v.startswith('--capture=')],camera_log=[v for v in log.splitlines() if any(k in v for k in ['CAMERA','SCENE','AdapterInfo'])]))
                manifest.write_text(json.dumps(records,indent=2)+'\n')
                print(dest,flush=True)


if __name__=='__main__':main()
