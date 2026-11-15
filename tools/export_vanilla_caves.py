#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Read-only export of an authorized Java26.3 overworld to RCV1.

Does not download/execute Minecraft, accept a EULA, modify a world, or contain
Minecraft code/assets. Requires completed chunks; absent chunks fail closed.
Run on a closed, unmodified reference world with structures disabled. Metadata
and region hashes establish provenance, not proof that a save is unmodified.
"""
import argparse
import csv
import gzip
import hashlib
import json
import math
import struct
import zlib
from pathlib import Path

import numpy as np


class NBT:
    """Small independent big-endian NBT reader; only standard binary tags."""
    def __init__(self, data):
        self.data, self.offset = data, 0

    def take(self, n):
        assert 0 <= n <= 64*1024*1024 and self.offset+n <= len(self.data), 'truncated NBT'
        v = self.data[self.offset:self.offset+n]
        self.offset += n
        return v

    def number(self, code):
        return struct.unpack('>'+code, self.take(struct.calcsize(code)))[0]

    def string(self):
        return self.take(self.number('H')).decode('utf-8')

    def payload(self, tag, depth=0):
        assert depth < 64, 'NBT nesting limit'
        if 1 <= tag <= 6:
            return self.number({1:'b',2:'h',3:'i',4:'q',5:'f',6:'d'}[tag])
        if tag == 8:
            return self.string()
        if tag == 10:
            result = {}
            while (kind := self.number('B')) != 0:
                name = self.string()
                result[name] = self.payload(kind, depth+1)
            return result
        if tag == 9:
            kind, length = self.number('B'), self.number('i')
            assert 0 <= length <= 4*1024*1024
            return [self.payload(kind, depth+1) for _ in range(length)]
        if tag in [7,11,12]:
            length = self.number('i')
            assert 0 <= length <= 8*1024*1024
            dtype = {7:'i1',11:'>i4',12:'>i8'}[tag]
            return np.frombuffer(self.take(length*np.dtype(dtype).itemsize), dtype=dtype)
        raise ValueError(f'unsupported NBT tag {tag}')

    def root(self):
        assert self.number('B') == 10, 'NBT root must be a compound'
        self.string()
        return self.payload(10)


GROUND = set('stone granite diorite andesite deepslate tuff calcite dripstone_block '
             'dirt coarse_dirt rooted_dirt podzol mycelium grass_block mud packed_mud '
             'sand red_sand gravel clay bedrock snow_block sandstone red_sandstone '
             'terracotta moss_block'.split())
OBSTACLE = set('short_grass tall_grass fern large_fern dead_bush vine cave_vines '
               'cave_vines_plant glow_lichen spore_blossom pointed_dripstone '
               'small_dripleaf big_dripleaf big_dripleaf_stem hanging_roots '
               'snow waterlily lily_pad sugar_cane seagrass tall_seagrass kelp kelp_plant '
               'dandelion poppy blue_orchid allium azure_bluet oxeye_daisy cornflower '
               'lily_of_the_valley wither_rose torch wall_torch red_mushroom brown_mushroom '
               'small_amethyst_bud medium_amethyst_bud large_amethyst_bud amethyst_cluster '
               'pink_petals wildflowers leaf_litter firefly_bush red_shrub shelf_mushroom '
               'sculk_vein sweet_berry_bush cobweb'.split())


def classify(entry):
    # 26.3 supports compact strings and renamed Name/Properties to id/properties.
    name = entry if isinstance(entry,str) else entry.get('id', entry.get('Name'))
    assert name and name.startswith('minecraft:'), f'non-vanilla palette entry: {entry}'
    name = name.split(':')[1]
    if name in ['air','cave_air','void_air']:
        code = 1
    elif name == 'water':
        code = 2
    elif name == 'lava':
        code = 3
    elif name in OBSTACLE or name.endswith(('_stairs','_slab','_fence','_wall','_sapling','_rail','_carpet')):
        code = 4
    else:
        code = 0
    ground = name in GROUND or name.endswith(('_ore','_terracotta'))
    return code, ground


def chunk(world, cx, cz, provenance):
    path = world/'region'/f'r.{cx//32}.{cz//32}.mca'
    if str(path) not in provenance:
        provenance[str(path)] = hashlib.sha256(path.read_bytes()).hexdigest()
    with path.open('rb') as f:
        f.seek(((cz%32)*32+cx%32)*4)
        loc = int.from_bytes(f.read(4),'big')
        assert loc>>8, f'missing generated chunk {cx},{cz}'
        f.seek((loc>>8)*4096)
        size = int.from_bytes(f.read(4),'big')
        method = f.read(1)[0]
        if method&128:
            data = (path.parent/f'c.{cx}.{cz}.mcc').read_bytes()
        else:
            assert 1 < size <= (loc&255)*4096-4
            data = f.read(size-1)
    method &= 127
    if method == 1:
        data = gzip.decompress(data)
    elif method == 2:
        data = zlib.decompress(data)
    elif method != 3:
        raise ValueError(f'unsupported Anvil compression {method}')
    root = NBT(data).root()
    assert root.get('Status',root.get('status')) in ['minecraft:full','full']
    assert root['xPos'] == cx and root['zPos'] == cz
    cells = np.ones((16,16,384),np.uint8)
    heights = np.full((16,16),-64,np.int16)
    for section in root['sections']:
        bottom = int(section['Y'])*16+64
        if not 0 <= bottom < 384 or 'block_states' not in section:
            continue
        states = section['block_states']
        palette = [classify(e) for e in states['palette']]
        codes, ground = map(np.array, zip(*palette))
        if len(palette) == 1:
            indices = np.zeros(4096,np.int32)
        else:
            bits = max(4,math.ceil(math.log2(len(palette))))
            per_long = 64//bits
            words = np.asarray(states['data'],dtype=np.uint64)
            i = np.arange(4096,dtype=np.uint64)
            assert len(words) == (4096+per_long-1)//per_long
            indices = ((words[i//per_long] >> ((i%per_long)*bits)) & ((1<<bits)-1)).astype(np.int32)
            assert indices.max() < len(palette)
        # Anvil index Y,Z,X -> common Z,X,Y.
        voxels = indices.reshape(16,16,16).transpose(1,2,0)
        cells[:,:,bottom:bottom+16] = codes[voxels]
        solid_ground = ground[voxels]
        for y in range(16):
            mask = solid_ground[:,:,y]
            heights[mask] = np.maximum(heights[mask],bottom+y-64)
    return cells, heights


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--world',type=Path,required=True)
    p.add_argument('--plan',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    a = p.parse_args()
    world = a.world.resolve()
    assert not a.output.resolve().is_relative_to(world), 'exports must be outside the reference world'
    data = NBT(gzip.decompress((world/'level.dat').read_bytes())).root()['Data']
    assert data['Version']['Name'] == '26.3', 'reference must be Java26.3 stable'
    seed = int(data['WorldGenSettings']['seed']) & ((1<<64)-1)
    a.output.mkdir(parents=True,exist_ok=True)
    provenance = {}
    jobs = [r for r in csv.DictReader(a.plan.open()) if int(r['seed']) == seed]
    assert jobs, 'no sample windows for this world seed'
    for row in jobs:
        x,z,n = map(int,[row['x'],row['z'],row['n']])
        assert n%16 == x%16 == z%16 == 0 and 0<n<=256
        cells = np.zeros((n,n,384),np.uint8)
        surface = np.zeros((n,n),np.int16)
        for dz in range(0,n,16):
            for dx in range(0,n,16):
                c,h = chunk(world,(x+dx)//16,(z+dz)//16,provenance)
                cells[dz:dz+16,dx:dx+16] = c
                surface[dz:dz+16,dx:dx+16] = h
        path = a.output/(row['id']+'.rcv')
        with path.open('xb') as out:
            out.write(b'RCV1'+struct.pack('<hhhQii',n,384,-64,seed,x,z))
            out.write(surface.astype('<i2').tobytes())
            out.write(cells.tobytes())
        print(path,flush=True)
    (a.output/f'vanilla-{seed}-provenance.json').write_text(json.dumps(dict(
        version='26.3',seed=str(seed),level_dat_sha256=hashlib.sha256((world/'level.dat').read_bytes()).hexdigest(),
        regions=provenance,exporter_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        caveat='Requires authorized unmodified completed vanilla worlds; metadata alone cannot prove provenance.'),indent=2)+'\n')


if __name__=='__main__':
    main()
