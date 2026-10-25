#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Capture matched Slice21 fixtures through the production shaders (desktop required)."""
import argparse
from pathlib import Path
import subprocess
import tempfile
from generate_textures import png

SCENES = [
    ('sunrise', 'plains', 0), ('morning', 'plains', 1000),
    ('noon', 'plains', 6000), ('sunset', 'plains', 12000),
    ('midnight', 'plains', 18000), ('night-torches', 'plains', 18500),
    ('cave-day', 'cave', 6000), ('cave-night', 'cave', 18000),
]

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--executable', type=Path, default=Path('target/debug/examples/day_night_gallery'))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for name, scene, units in SCENES:
        destination = args.output / f'slice21-{name}.png'
        if destination.exists():
            parser.error(f'Refusing to overwrite {destination}')
        with tempfile.TemporaryDirectory() as scratch:
            ppm = Path(scratch) / 'capture.ppm'
            subprocess.run([str(args.executable.resolve()), scene, '2', f'--time={units}', f'--capture={ppm}'], check=True, timeout=120)
            magic, size, maximum, rgb = ppm.read_bytes().split(b'\n', 3)
            assert magic == b'P6' and maximum == b'255'
            width, height = map(int, size.split())
            assert len(rgb) == width * height * 3
            assert len(set(rgb)) > 16, 'Renderer did not finish initialization'
            rgba = b''.join(rgb[i:i+3] + b'\xff' for i in range(0, len(rgb), 3))
            destination.write_bytes(png(width, height, rgba, compression=9))
            print(destination, flush=True)
