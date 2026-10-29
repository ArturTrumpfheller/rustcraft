#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Compile original editable pixel grids; Python standard library only.

Run from any directory. --check compares every generated artifact without writing.
No texture generation, image decoding, file lookup or Python is needed at runtime.
"""
import argparse
import json
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parent.parent / 'assets' / 'terrain'
NAMES = ['grass_top', 'grass_side', 'dirt', 'stone', 'bark', 'end_grain', 'leaves', 'torch', 'sand', 'snow_top', 'snow_side']


def png(width, height, data, compression=0):
    def chunk(kind, payload):
        return struct.pack('>I', len(payload)) + kind + payload + struct.pack('>I', zlib.crc32(kind + payload))
    scan = b''.join(b'\0' + data[y * width * 4:(y + 1) * width * 4] for y in range(height))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(scan, level=compression)) + chunk(b'IEND', b''))


def generate():
    source = json.loads((ROOT / 'source.json').read_text())
    assert source['license'] == 'MIT OR Apache-2.0'
    assert (source['tile_size'], source['pitch'], source['padding'], source['atlas_size']) == (16, 32, 8, [128, 96])
    assert [t['name'] for t in source['tiles']] == NAMES
    atlas = bytearray(bytes([255, 0, 255, 255]) * (128 * 96))
    result = {}
    for i, tile in enumerate(source['tiles']):
        rows = tile['rows']
        assert len(rows) == 16 and all(len(row) == 16 for row in rows)
        palette = {k: bytes.fromhex(v.removeprefix('#')) + b'\xff' for k, v in tile['palette'].items()}
        assert all(len(k) == 1 and len(v) == 4 for k, v in palette.items())
        pixels = b''.join(palette[p] for row in rows for p in row)
        result[tile['name'] + '.png'] = png(16, 16, pixels)
        # Extrude edge texels into every gutter, including corners. No mipmaps.
        for y in range(32):
            for x in range(32):
                sx, sy = min(15, max(0, x - 8)), min(15, max(0, y - 8))
                dest = (((i // 4) * 32 + y) * 128 + (i % 4) * 32 + x) * 4
                atlas[dest:dest + 4] = pixels[(sy * 16 + sx) * 4:(sy * 16 + sx + 1) * 4]
    result['atlas.rgba'] = bytes(atlas)
    result['atlas.png'] = png(128, 96, atlas)
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    for name, data in generate().items():
        path = ROOT / name
        if args.check:
            if not path.exists() or path.read_bytes() != data:
                parser.exit(1, f'Out of date: {path}; run tools/generate_textures.py\n')
        else:
            path.write_bytes(data)
    print('Texture artifacts match source.' if args.check else 'Texture artifacts generated.')
