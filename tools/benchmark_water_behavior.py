#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Run identical Slice22/23 core harnesses sequentially, recording raw CPU/RSS.

Linux/macOS measurement helper; game and Rust harness remain cross-platform.
No renderer is involved and these results make no native GPU/FPS claim.
"""
import argparse
import csv
import io
from pathlib import Path
import resource
import statistics
import subprocess
import sys

SCENES = ['dry', 'pool', 'several', 'waterfall', 'obstacles', 'holes',
          'drain-edits', 'boundary', 'settled']


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('before', type=Path)
    p.add_argument('after', type=Path)
    p.add_argument('output', type=Path)
    p.add_argument('--repeats', type=int, default=9)
    args = p.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    rows = []
    for scene in SCENES:
        for label, binary in [('slice22', args.before), ('slice23', args.after)]:
            result = subprocess.run([sys.executable, str(Path(__file__).resolve()),
                                     '--child', str(binary.resolve()),
                                     f'--scene={scene}', f'--repeats={args.repeats}'],
                                    capture_output=True, text=True, check=True, timeout=120)
            (args.output / f'{label}-{scene}.csv').write_text(result.stdout)
            (args.output / f'{label}-{scene}-resource.txt').write_text(result.stderr)
            samples = list(csv.DictReader(io.StringIO(result.stdout)))
            row = {'phase': label, 'scene': scene, 'repeats': len(samples)}
            for key in samples[0]:
                if key not in ['scene', 'repeat']:
                    row[key] = statistics.median(float(s[key]) for s in samples)
            rows.append(row)
            print(label, scene, row['tick_total_us'], flush=True)
    with (args.output / 'summary.csv').open('w', newline='') as out:
        writer = csv.DictWriter(out, fieldnames=list(rows[0]), lineterminator='\n')
        writer.writeheader()
        writer.writerows(rows)


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == '--child':
        result = subprocess.run(sys.argv[2:], check=False)
        usage = resource.getrusage(resource.RUSAGE_CHILDREN)
        print(f'user_s={usage.ru_utime:.4f} system_s={usage.ru_stime:.4f} '
              f'rss_kib={usage.ru_maxrss}', file=sys.stderr)
        sys.exit(result.returncode)
    main()
