#!/usr/bin/env python3
"""Compare prebuilt Slice 17/18 gallery binaries (no build, capture or dependencies)."""
import argparse
import csv
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('before', type=Path)
    parser.add_argument('after', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--frames', type=int, default=180)
    args = parser.parse_args()
    if not 1 <= args.frames <= 2000:
        parser.error('frames must be 1..2000')
    cases = [('plains', 2, []), ('forest', 2, []), ('uneven', 3, []),
             ('cave', 2, []), ('room', 1, []), ('plains', 1, []),
             ('plains', 3, []), ('plains', 3, ['--motion']),
             ('plains', 3, ['--motion', '--cycle'])]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('w', newline='', encoding='utf-8') as stream:
        writer = None
        for scene, radius, flags in cases:
            for phase, binary in [('before', args.before), ('after', args.after)]:
                command = [str(binary.resolve()), scene, str(radius),
                           f'--frames={args.frames}', *flags]
                if phase == 'before':
                    command.append('--baseline')
                result = subprocess.run(command, capture_output=True, text=True,
                                        check=True, timeout=180)
                if 'ERROR' in result.stderr:
                    raise RuntimeError(result.stderr)
                line = next(line for line in result.stdout.splitlines()
                            if line.startswith('ATMOSPHERE_FRAME '))
                row = dict(item.split('=', 1) for item in line.split()[1:])
                row = {'phase': phase, **row}
                if writer is None:
                    writer = csv.DictWriter(stream, fieldnames=list(row), lineterminator="\n")
                    writer.writeheader()
                writer.writerow(row)
                stream.flush()
                print(line, flush=True)


if __name__ == '__main__':
    main()
