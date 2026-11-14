#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Prepare a pinned, isolated reference workspace; no files enter the game.
Run the printed exporter commands separately. Refuses an existing destination.
"""
import argparse
from pathlib import Path
import shutil
import subprocess
HERE=Path(__file__).resolve().parent
HORIZON='0de893227f0dc4ad7bbbedee37744e007df04151'
METADATA='538b2b167248c648b2198f2c0d56eced10dfc0cf'
def run(*args,cwd):subprocess.run(args,cwd=cwd,check=True)
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('destination',type=Path);a=p.parse_args();root=a.destination.resolve();root.mkdir(parents=True,exist_ok=False)
 for name,url,ref,paths in [
  ('horizon','https://github.com/horizon-rust/horizon',HORIZON,['crates/horizon-core','crates/horizon-world','crates/horizon-threads']),
  ('metadata','https://github.com/misode/mcmeta',METADATA,['data/minecraft/worldgen/'+s for s in ['noise','density_function','noise_settings','carver','biome']])]:
  (root/name).mkdir()
  run('git','init',cwd=root/name)
  run('git','remote','add','origin',url,cwd=root/name)
  run('git','fetch','--depth=1','--filter=blob:none','origin',ref,cwd=root/name)
  run('git','sparse-checkout','init','--cone',cwd=root/name)
  run('git','sparse-checkout','set',*paths,cwd=root/name)
  run('git','checkout','--detach','FETCH_HEAD',cwd=root/name)
  assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root/name,text=True).strip()==ref
 work=root/'run';work.mkdir()
 for name in ['horizon-core','horizon-world','horizon-threads']:shutil.copytree(root/'horizon/crates'/name,work/'crates'/name)
 for name in ['LICENSE','THIRD_PARTY_NOTICES.md','Cargo.lock']:shutil.copy2(root/'horizon'/name,work/name)
 shutil.copy2(HERE/'Cargo.reference.toml',work/'Cargo.toml')
 for name in ['cave_reference','noise_reference','terrain_reference']:shutil.copy2(HERE/(name+'.rs'),work/'crates/horizon-world/examples'/('rustcraft_'+name+'.rs'))
 print('Build in',work,': cargo build --release -p horizon-world --example rustcraft_cave_reference --example rustcraft_noise_reference --example rustcraft_terrain_reference')
 print('Then use --locked. Cave exporter arguments: METADATA/data/minecraft/worldgen PLAN.csv OUTPUT_DIRECTORY')
 print('Noise exporter argument: METADATA/data/minecraft/worldgen/noise')
if __name__=='__main__':main()
