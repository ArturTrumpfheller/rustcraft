#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Paired cave metrics using a fixed, cave-disabled geological denominator.
Deep metrics use the same masked domain in both versions. Window/depth cuts
truncate components and routes; they are not whole-world topology claims.
"""
import argparse, csv, hashlib, json
from pathlib import Path
import numpy as np
from scipy import ndimage
from analyze_cave_volumes import load, graph, routes, run_lengths


def metrics(cells, geology, domain):
    dry = cells == 1
    cavity = geology & domain & ((cells == 1) | (cells == 2) | (cells == 3))
    labels, _ = ndimage.label(cavity)
    sizes = np.bincount(labels.ravel())[1:]
    floor = dry & geology & domain
    floor[:, :, 0] = floor[:, :, -1] = False
    floor[:, :, 1:] &= cells[:, :, :-1] == 0
    floor[:, :, :-1] &= dry[:, :, 1:]
    g, coords = graph(floor, dry)
    _, floor_sizes, route = routes(g)
    widths = np.minimum(run_lengths(cavity, 0), run_lengths(cavity, 1))[cavity]
    heights = run_lengths(cavity, 2)[cavity]
    return dict(cavity_cells=int(cavity.sum()), components=int(len(sizes)),
                largest_cavity=int(sizes.max(initial=0)),
                component_size_bins=np.histogram(sizes, [0, 128, 2048, 32768, np.inf])[0].tolist(),
                supported_floor_cells=len(coords), floor_components=len(floor_sizes),
                largest_floor=int(floor_sizes.max(initial=0)), supported_route=route,
                width_bins=np.histogram(widths, [0, 3, 6, 12, 24, np.inf])[0].tolist(),
                height_bins=np.histogram(heights, [0, 3, 6, 12, 24, np.inf])[0].tolist())


def compare(before, after, base, ground, meta):
    y = np.arange(before.shape[2])[None, None, :] + meta['min_y']
    depth = ground[:, :, None] - y
    geology = base == 0
    distance = ndimage.distance_transform_edt(geology)
    valid = y >= meta['min_y'] + 4
    deep = geology & (depth >= 24) & (distance > 24) & valid
    deep[:24] = deep[-24:] = False
    deep[:, :24] = deep[:, -24:] = False
    changed = (before != after) & geology
    new_air = (after == 1) & (before != 1) & geology
    result = dict(deep_geology_cells=int(deep.sum()), deep_changed_cells=int(changed[deep].sum()),
        new_geological_air=int(new_air.sum()), changed_geology_cells=int(changed.sum()),
        changed_distance_bins=np.histogram(distance[changed], [0, 4, 8, 12, 16, 24, np.inf])[0].tolist(),
        changed_cover_bins=np.histogram(depth[changed], [-np.inf, 4, 8, 16, 24, np.inf])[0].tolist(),
        deep_before_sha256=hashlib.sha256(before[deep].tobytes()).hexdigest(),
        deep_after_sha256=hashlib.sha256(after[deep].tobytes()).hexdigest())
    # Fixed geological cover8 domain measures near-surface losses as well.
    result['v15'] = metrics(before, geology, (depth >= 8) & valid)
    result['v16'] = metrics(after, geology, (depth >= 8) & valid)
    result['deep_v15'] = metrics(before, geology, deep)
    result['deep_v16'] = result['deep_v15'] if result['deep_changed_cells'] == 0 else metrics(after, geology, deep)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ['plan', 'base', 'v15', 'v16', 'output']:
        parser.add_argument('--'+key, required=True, type=Path)
    args = parser.parse_args()
    with args.output.open('x') as out:
        for row in csv.DictReader(args.plan.open()):
            name = row['id']+'.rcv'
            before, _, meta = load(args.v15/name)
            after, _, new_meta = load(args.v16/name)
            base, ground, base_meta = load(args.base/name)
            for key in ['seed', 'x', 'z', 'n', 'height', 'min_y']:
                assert meta[key] == new_meta[key] == base_meta[key]
            result = compare(before, after, base, ground, meta)
            result.update(id=row['id'], stratum=row['stratum'], **meta,
                          v16_sha256=new_meta['sha256'], base_sha256=base_meta['sha256'])
            out.write(json.dumps(result, separators=(',', ':'))+'\n')
            out.flush()
            print(row['id'], result['deep_changed_cells'], result['new_geological_air'], flush=True)

if __name__ == '__main__':
    main()
