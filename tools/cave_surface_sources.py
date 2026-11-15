#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Build disposable v15 instrumentation: uncaved geology and surface source tags.
No instrumentation enters the application. Frozen v15 is the attribution subject.
Tags identify exact carver/noodle/main branches; main-family bits are contributing
negative interpolation-corner families, not exclusive causal shares.
"""
import argparse,io,os,shutil,subprocess,tarfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
BASE='577b199f23a0995f765bdf81fe62f5a9f8414fe7'
def prepare(dest):
    dest.mkdir(parents=True,exist_ok=False)
    data=subprocess.check_output(['git','archive',BASE,'Cargo.toml','Cargo.lock','rust-toolchain.toml','crates/voxel_core','client/Cargo.toml','client/src/main.rs'],cwd=ROOT)
    with tarfile.open(fileobj=io.BytesIO(data)) as t:t.extractall(dest,filter='data')
    p=dest/'crates/voxel_core/src/worldgen/caves_v3.rs';s=p.read_text().replace('    Historical(caves::Plan),','    Disabled,\n    Historical(caves::Plan),').replace('        if matches!(g.config.version, 14 | 15) {','        if std::env::var_os("CAVE_ANALYSIS_NO_CAVES").is_some() { return Self::Disabled; }\n        if matches!(g.config.version, 14 | 15) {',1)
    s=s.replace('            Self::Historical(c) => c.contains(p, height),','            Self::Disabled => false,\n            Self::Historical(c) => c.contains(p, height),')
    s=s.replace('            Self::Historical(plan) => plan.carve(c, p, height),','            Self::Disabled => {},\n            Self::Historical(plan) => plan.carve(c, p, height),').replace('            Self::Historical(plan) => plan.carve_if(c, p, height, permitted),','            Self::Disabled => {},\n            Self::Historical(plan) => plan.carve_if(c, p, height, permitted),')
    s+='''
// Scratch-only exact branch probes; never part of production sources.
pub fn surface_sources(g: &Generator, min: [i32;2], max: [i32;2], p: BlockPos, h:i32) -> u16 {
    let plan = Plan::new(g,min,max); plan.source_at(p,h)
}
pub struct SourcePlan(Plan);
impl SourcePlan {
    pub fn new(g:&Generator,min:[i32;2],max:[i32;2])->Self{Self(Plan::new(g,min,max))}
    pub fn at(&self,p:BlockPos,h:i32)->u16{self.0.source_at(p,h)}
}
impl Plan {
    fn source_at(&self,p:BlockPos,h:i32)->u16 {
        if !Self::allowed(p.0,h,true){return 0;}
        let mut bits=self.network.source_at(p.0);
        for shape in &self.shapes {
            if shape.contains(p.0){bits |= if shape.canyon(){256}else{128};}
        }
        bits
    }
}
''';p.write_text(s)
    p=dest/'crates/voxel_core/src/worldgen/caves_v3/carvers.rs';s=p.read_text();s+='\nimpl Ellipsoid { pub(super) fn canyon(&self)->bool {self.floor == -4096} }\n';p.write_text(s)
    p=dest/'crates/voxel_core/src/worldgen/caves_v3/density.rs';s=p.read_text().replace('[i32; 5]','[i32; 6]')
    s=s.replace('            at(19, 8. / 3., 8. / 3.),\n        ]','            at(19, 8. / 3., 8. / 3.),\n            0.,\n        ]')
    s+='''
impl Network {
    pub(super) fn source_at(&self,p:[i32;3])->u16 {
        let lattice=self.lattice(p[1]);let v=lattice.sample(p);let mut bits=0;
        if v[0]<0 {
            for c in lattice.corners(lattice.cell(p)) {if c[0]<0 {bits|=(c[5]/Q) as u16;}}
        }
        if v[1]>=0 && i64::from(v[2])+i64::from(v[3].abs().max(v[4].abs()))*3/2<0 {bits|=64;}
        bits
    }
}
''';p.write_text(s)
    p=dest/'crates/voxel_core/src/worldgen/caves_v3/density/refinement.rs';s=p.read_text().replace('[i32; 5]','[i32; 6]')
    a=s.index('        let entrance = (at(2');b=s.index('        let sloped =',a)
    s=s[:a]+'''        let broad = at(2,0.75,0.5)+0.31+0.16*upland-0.08*exposure
            +0.3*((-31.-f64::from(p[1]))/40.).clamp(0.,1.) as f32;
        let thin = ridges + upland*(covered-ridges);
        let entrance=broad.min(thin);
        let mut tags=(if broad<0. {2} else {0}) | (if thin<0. {4}else{0});
'''+s[b:]
    s=s.replace('            sloped.min(5. * entrance)','            if sloped<0. {tags|=32;}\n            sloped.min(5. * entrance)',1)
    s=s.replace('            sloped.min(5. * entrance)', '            if std::env::var_os("CAVE_ANALYSIS_NO_TERRAIN_INTERSECTION").is_some() {5. * entrance} else {sloped.min(5. * entrance)}', 1)
    s=s.replace('            let a = cheese.min(entrance);','            if cheese<0. {tags|=1;}\n            if spaghetti2+rough<0. {tags|=8;}\n            let a = cheese.min(entrance);',1)
    s=s.replace('            (a.min(b) - overlap * overlap / 0.24).max(', '            if a.min(b)>=0. && a.min(b)-overlap*overlap/0.24<0. {tags|=16;}\n            (a.min(b) - overlap * overlap / 0.24).max(',1)
    s=s.replace('            at(19, 8. / 3., 8. / 3.),\n        ]','            at(19, 8. / 3., 8. / 3.),\n            tags as f32,\n        ]')
    s=s.replace('        [\n            main,','        let mut values = [\n            main,',1).replace('.map(|v| (v.clamp(-32., 32.) * Q as f32).round() as i32)\n    }','.map(|v| (v.clamp(-32., 32.) * Q as f32).round() as i32);\n        values[5]=tags*Q;\n        values\n    }',1);p.write_text(s)
    p=dest/'crates/voxel_core/examples/cave_volume.rs';s=p.read_text().replace('                                } else if b.is_solid() {','                                } else if b.is_solid() && (std::env::var_os("CAVE_ANALYSIS_NO_CAVES").is_none() || matches!(b,Block::Stone|Block::Dirt|Block::Grass|Block::Sand|Block::SnowyGround)) {');p.write_text(s)
    p=dest/'crates/voxel_core/examples/surface_sources.rs';p.write_text('''
use std::{env,fs,io::Write,path::Path};
use voxel_core::{BlockPos,worldgen::{Generator,GenerationConfig,caves_v3::SourcePlan}};
fn main(){let a:Vec<_>=env::args().collect();assert_eq!(a.len(),5);fs::create_dir_all(&a[4]).unwrap();
for line in fs::read_to_string(&a[1]).unwrap().lines().skip(1){let r:Vec<_>=line.split(',').collect();let seed=r[1].parse().unwrap();let x:i32=r[2].parse().unwrap();let z:i32=r[3].parse().unwrap();let n:usize=r[4].parse().unwrap();
let base=fs::read(Path::new(&a[2]).join(format!("{}.rcv",r[0]))).unwrap();let old=fs::read(Path::new(&a[3]).join(format!("{}.rcv",r[0]))).unwrap();let off=26+n*n*2;assert_eq!(base.len(),off+n*n*256);let cells=&base[off..];let air=&old[off..];
let g=Generator::new(GenerationConfig{seed,version:15}).unwrap();let plan=SourcePlan::new(&g,[x,z],[x+n as i32-1,z+n as i32-1]);let mut tags=vec![0u16;n*n*256];
for zz in 1..n-1{for xx in 1..n-1{let col=zz*n+xx;let h=i16::from_le_bytes(base[26+col*2..28+col*2].try_into().unwrap()) as i32;
for yy in 4..255 {let i=col*256+yy;if cells[i]!=0 || air[i]!=1{continue;}let adjacent=[i-1,i+1,i-256,i+256,i-n*256,i+n*256].iter().any(|&j|cells[j]==1);if adjacent{tags[i]=plan.at(BlockPos([x+xx as i32,yy as i32-64,z+zz as i32]),h);}}}}
let path=Path::new(&a[4]).join(format!("{}.src",r[0]));let mut f=fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap();for t in tags{f.write_all(&t.to_le_bytes()).unwrap();}println!("{}",r[0]);}}
''')
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--workspace',required=True,type=Path);a=p.parse_args();prepare(a.workspace.resolve())
if __name__=='__main__':main()
