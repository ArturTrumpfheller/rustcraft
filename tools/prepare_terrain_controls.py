#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Copy current core to an isolated cave-disabled analysis workspace.

No runtime switch is introduced into production. Both v16/v17 keep normal
terrain density, water and ecology; geological export excludes tree obstacles.
"""
import argparse
import shutil
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('destination',type=Path);a=p.parse_args()
    dest=a.destination.resolve();dest.mkdir(parents=True,exist_ok=False)
    for f in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','client/Cargo.toml','client/src/main.rs']:
        target=dest/f;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/f,target)
    shutil.copytree(ROOT/'crates/voxel_core',dest/'crates/voxel_core')
    path=dest/'crates/voxel_core/src/worldgen/caves_v3.rs'
    s=path.read_text().replace('    Historical(caves::Plan),','    Disabled,\n    Historical(caves::Plan),')
    s=s.replace('        if matches!(g.config.version, 14..=17) {','        if matches!(g.config.version,16 | 17) { return Self::Disabled; }\n        if matches!(g.config.version, 14..=17) {',1)
    s=s.replace('            Self::Historical(c) => c.contains(p, height),','            Self::Disabled => false,\n            Self::Historical(c) => c.contains(p, height),')
    s=s.replace('            Self::Historical(plan) => plan.carve(c, p, height),','            Self::Disabled => {},\n            Self::Historical(plan) => plan.carve(c, p, height),')
    s=s.replace('            Self::Historical(plan) => plan.carve_if(c, p, height, permitted),','            Self::Disabled => {},\n            Self::Historical(plan) => plan.carve_if(c, p, height, permitted),')
    path.write_text(s)
    path=dest/'crates/voxel_core/examples/cave_volume.rs'
    s=path.read_text().replace('                                } else if b.is_solid() {','                                } else if matches!(b,Block::Stone|Block::Dirt|Block::Grass|Block::Sand|Block::SnowyGround) {')
    path.write_text(s)
    print(dest)


if __name__=='__main__':main()
