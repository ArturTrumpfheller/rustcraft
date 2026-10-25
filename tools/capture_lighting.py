#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Capture six matched production-renderer fixtures (requires a desktop/display).
Build first: cargo build --locked -p rustcraft --example lighting_gallery
No image libraries or PNG runtime feature are needed by RustCraft.
"""
import argparse
from pathlib import Path
import subprocess
import tempfile
from generate_textures import png

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--executable', type=Path, default=Path('target/debug/examples/lighting_gallery'))
    parser.add_argument('--scenes', nargs='+', default=['dark', 'one', 'several', 'removed', 'wall', 'seam'])
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for scene in args.scenes:
        dest = args.output / f'slice20-{scene}.png'
        if dest.exists():
            parser.error(f'Refusing to overwrite {dest}')
        with tempfile.TemporaryDirectory() as scratch:
            ppm = Path(scratch) / 'capture.ppm'
            subprocess.run([str(args.executable.resolve()), scene, str(ppm)], check=True, timeout=120)
            magic, size, maximum, rgb = ppm.read_bytes().split(b'\n', 3)
            assert magic == b'P6' and maximum == b'255'
            width, height = map(int, size.split())
            assert len(rgb) == width * height * 3
            rgba = b''.join(rgb[i:i+3]+b'\xff' for i in range(0, len(rgb), 3))
            dest.write_bytes(png(width, height, rgba, compression=9))
            print(dest)
