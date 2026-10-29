#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Capture actual v8 geometry with RustCraft's production terrain/water shaders."""
import argparse
import subprocess
from pathlib import Path
from generate_textures import png

# name, seed, center X/Z, eye X/Y/Z, view direction, radius, extra fixture flags
SCENES = [
    ('pond', 0, (8498, -8668), (8513, 14, -8653), (-15, -11, -15), 3, []),
    ('lake', 42, (-434, 31), (-402, 32, 63), (-32, -29, -32), 4, []),
    ('river', 2026, (194, -239), (150, 30, -211), (35, -27, -28), 7, []),
    ('connection', 2026, (332, -183), (305, 33, -146), (27, -30, -37), 5, []),
    ('coast', 2026, (380, 35), (359, 32, 65), (40, -29, -40), 6, []),
    ('underground', 2026, (-208, -670), (-204, -47, -662), (4, -5, -8), 3, ['--cave-torches']),
    ('rocky-shore', 2026, (-97, -466), (-90, 6, -465), (-7, -3, -1), 3, []),
]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--executable', type=Path, default=Path('target/debug/examples/terrain_gallery'))
    parser.add_argument('--scene', choices=[s[0] for s in SCENES])
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for name, seed, center, eye, direction, radius, flags in SCENES:
        if args.scene and name != args.scene:
            continue
        ppm = args.output / f'{name}.ppm'
        command = [str(args.executable.resolve()), str(seed), *map(str, center), '0', '--generator=8',
                   '--eye=' + ','.join(map(str, eye)), '--direction=' + ','.join(map(str, direction)),
                   f'--radius={radius}', '--survey-fog', f'--capture={ppm.resolve()}', *flags]
        result = subprocess.run(command, capture_output=True, text=True, check=True, timeout=300)
        (args.output / f'{name}.log').write_text(result.stdout + result.stderr)
        magic, size, maximum, rgb = ppm.read_bytes().split(b'\n', 3)
        width, height = map(int, size.split())
        assert magic == b'P6' and maximum == b'255' and len(rgb) == width * height * 3
        assert len(set(rgb)) > 16, 'renderer capture is blank'
        rgba = b''.join(rgb[i:i+3] + b'\xff' for i in range(0, len(rgb), 3))
        destination = args.output / f'{name}.png'
        destination.write_bytes(png(width, height, rgba, compression=9))
        ppm.unlink()
        print(destination, flush=True)

if __name__ == '__main__':
    main()
