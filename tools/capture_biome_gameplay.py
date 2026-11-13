#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Actual X11 production-client input, walking, F3 and normal save/quit captures.
Uses new isolated catalogs only; no gallery renderer or user saves are involved.
Requires xdotool and Pillow ImageGrab in the development environment.
"""
import argparse,hashlib,json,os,re,subprocess,time
from pathlib import Path
from PIL import ImageGrab

def xdo(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True).strip()
def key(window,name):
 xdo('keydown',name);time.sleep(.6);xdo('keyup',name);time.sleep(.35)
def click(x,y):
 xdo('mousemove',x,y);xdo('mousedown',1);time.sleep(.2);xdo('mouseup',1);time.sleep(.5)
def until(condition,timeout=60):
 end=time.monotonic()+timeout
 while time.monotonic()<end:
  value=condition()
  if value:return value
  time.sleep(.2)
 raise TimeoutError('Production client did not reach the requested state')
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--data-root',type=Path,required=True);p.add_argument('--cave-v3',action='store_true');p.add_argument('--executable',type=Path,default=Path('target/release/rustcraft'));a=p.parse_args()
 a.output.mkdir(parents=True,exist_ok=True);a.data_root.mkdir(parents=True,exist_ok=False);records=[]
 version=14 if a.cave_v3 else 12
 scenes=[('cave-entrance',2026,[5,-11],4)] if a.cave_v3 else [('birch',0,[-120,-252],4),('grove',18446744073709551615,[-101,-252],2)]
 for name,seed,chunk,radius in scenes:
  data=(a.data_root/name).resolve();data.mkdir();log_path=a.output/f'gameplay-{name}-r{radius}.log'
  with log_path.open('w') as log:
   proc=subprocess.Popen([str(a.executable.resolve())],stdout=log,stderr=log,env={**os.environ,'RUSTCRAFT_DATA_DIR':str(data),'RUSTCRAFT_WORLD_SEED':str(seed),'RUSTCRAFT_START_CHUNK':','.join(map(str,chunk)),'RUSTCRAFT_LOAD_RADIUS':str(radius)})
   try:
    def find_window():
     try:return xdo('search','--onlyvisible','--pid',proc.pid,'--name','RustCraft').splitlines()[0]
     except (subprocess.CalledProcessError,IndexError):return None
    window=until(find_window);xdo('windowsize',window,1280,720);xdo('windowmove',window,0,0);xdo('windowfocus','--sync',window);time.sleep(2)
    key(window,'Return');time.sleep(1)
    for _ in range(3):key(window,'Down')
    key(window,'Return');key(window,'Tab');key(window,'Tab');key(window,'Return')
    until(lambda:list((data/'worlds').glob('*/world.meta')));time.sleep(1)
    if a.cave_v3:click(350,558)
    else:
     for _ in range(3):key(window,'Down')
     key(window,'Return')
    expected_chunks=(radius*2+1)**2*16
    title=until(lambda:(t if f'Chunks: {expected_chunks} pending 0 dirty 0' in (t:=xdo('getwindowname',window)) and f'gen{version} radius {radius}' in t else None),timeout=90)
    time.sleep(2);key(window,'Escape');key(window,'Escape')
    before=xdo('getwindowname',window);xdo('keydown','w');time.sleep(2.5 if a.cave_v3 else 1);xdo('keyup','w');time.sleep(1);after=xdo('getwindowname',window)
    for debug in [False,True]:
     if debug:key(window,'F3');time.sleep(1)
     path=a.output/f'gameplay-{name}-r{radius}{"-f3" if debug else ""}.png';ImageGrab.grab(xdisplay=os.environ.get('DISPLAY')).save(path)
     records.append(dict(file=path.name,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),seed=seed,generator=version,radius=radius,start_chunk=chunk,development_panorama=False,production_client=True,title_ready=title,title_before_walk=before,title_after_walk=after,executable_sha256=hashlib.sha256(a.executable.read_bytes()).hexdigest()))
    if a.cave_v3:
     key(window,'F3');key(window,'6')
     xdo('mousemove_relative','--',0,350);time.sleep(1)
     xdo('mousedown',3);time.sleep(.3);xdo('mouseup',3);time.sleep(2)
     path=a.output/f'gameplay-{name}-r{radius}-torch.png';ImageGrab.grab(xdisplay=os.environ.get('DISPLAY')).save(path)
     records.append(dict(file=path.name,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),seed=seed,generator=version,radius=radius,start_chunk=chunk,development_panorama=False,production_client=True,normal_torch_placement_input=True,title_after_input=xdo('getwindowname',window),executable_sha256=hashlib.sha256(a.executable.read_bytes()).hexdigest()))
    key(window,'Escape')
    for _ in range(3):key(window,'Down')
    key(window,'Return');proc.wait(timeout=30);assert proc.returncode==0
    for r in records:
     if r['seed']==seed:r['normal_save_quit_exit_code']=proc.returncode
   finally:
    if proc.poll() is None:proc.terminate();proc.wait(timeout=10)
  (a.output/'gameplay-captures.json').write_text(json.dumps(records,indent=2)+'\n');print(name,after,flush=True)
if __name__=='__main__':main()
