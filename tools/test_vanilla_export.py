#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Handcrafted binary-format tests; these are not genuine Minecraft samples."""
import struct
import tempfile
import unittest
import zlib
from pathlib import Path

import numpy as np
from export_vanilla_caves import NBT, chunk, classify


def string(s):
    b=s.encode()
    return struct.pack('>H',len(b))+b


def tag(kind,name,payload):
    return bytes([kind])+string(name)+payload


class VanillaFormatTests(unittest.TestCase):
    def test_standard_nbt_arrays_and_signed_values(self):
        data = tag(10,'',tag(4,'seed',struct.pack('>q',-1))+
                   tag(12,'longs',struct.pack('>iQQ',2,0xffffffffffffffff,0x123456789abcdef0))+b'\0')
        root=NBT(data).root()
        self.assertEqual(root['seed'],-1)
        self.assertEqual(root['longs'].tolist(),[-1,0x123456789abcdef0])
        with self.assertRaises(AssertionError):
            NBT(data[:-1]).root()

    def test_modern_palette_and_anvil_axis_order(self):
        self.assertEqual(classify('minecraft:cave_air'),(1,False))
        self.assertEqual(classify({'id':'minecraft:water'}),(2,False))
        self.assertEqual(classify({'id':'minecraft:pointed_dripstone'}),(4,False))
        # Each X triplet at all Z/Y is Stone, Air, Water. Force padding at the
        # end of every 64-bit word; high bits test unsigned extraction.
        indices=np.arange(4096)%3
        words=[]
        for start in range(0,4096,16):
            words.append(sum(int(indices[start+i]) << (4*i) for i in range(16)))
        states=tag(9,'palette',bytes([8])+struct.pack('>i',3)+b''.join(map(string,
                   ['minecraft:stone','minecraft:air','minecraft:water'])))
        states+=tag(12,'data',struct.pack('>i',len(words))+b''.join(struct.pack('>Q',w) for w in words))
        section=tag(1,'Y',struct.pack('>b',-4))+tag(10,'block_states',states+b'\0')+b'\0'
        root=tag(10,'',tag(8,'Status',string('minecraft:full'))+tag(3,'xPos',struct.pack('>i',-1))+
                 tag(3,'zPos',struct.pack('>i',-2))+tag(9,'sections',bytes([10])+struct.pack('>i',1)+section)+b'\0')
        compressed=zlib.compress(root)
        record=struct.pack('>I',len(compressed)+1)+bytes([2])+compressed
        header=bytearray(8192)
        offset=((-2%32)*32+(-1%32))*4
        header[offset:offset+4]=struct.pack('>I',(2<<8)|1)
        with tempfile.TemporaryDirectory() as temp:
            world=Path(temp);(world/'region').mkdir()
            (world/'region/r.-1.-1.mca').write_bytes(header+record+bytes(4096-len(record)))
            cells,surface=chunk(world,-1,-2,{})
        for y,z,x in [(0,0,0),(15,15,15),(7,9,13),(0,0,1),(0,0,2)]:
            expected=[0,1,2][(y*256+z*16+x)%3]
            self.assertEqual(int(cells[z,x,y]),expected)
        self.assertTrue((surface<=-49).all())


if __name__=='__main__':
    unittest.main()
