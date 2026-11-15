#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Select cameras from final voxels, then capture untouched production Bevy geometry.
Bounded, deterministic inspection selection; not a distribution/quality benchmark.
"""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

import numpy as np
from scipy import ndimage
from scipy.sparse import csgraph
from analyze_cave_volumes import graph, load, run_lengths
from generate_textures import png


def interior(path, kind):
    cells, surface, meta = load(path)
    air = cells == 1
    y = np.arange(cells.shape[2])[None, None, :] + meta['min_y']
    depth = surface[:, :, None] - y
    floor = air.copy()
    floor[:, :, 1:] &= cells[:, :, :-1] == 0
    floor[:, :, :-1] &= air[:, :, 1:]
    floor[:, :, 0] = floor[:, :, -1] = False
    floor &= (depth >= 12) & (y >= -57)
    floor[:12] = floor[-12:] = False
    floor[:, :12] = floor[:, -12:] = False
    width = np.minimum(run_lengths(air, 0), run_lengths(air, 1))
    height = run_lengths(air, 2)
    lo, hi, hlo, hhi = {
        'small': (2, 6, 2, 6), 'medium': (5, 18, 3, 20),
        'cavern': (16, 128, 10, 128), 'network': (3, 20, 3, 20),
        'deep': (3, 128, 3, 128), 'vertical': (3, 32, 16, 128),
    }[kind]
    floor &= (width >= lo) & (width <= hi) & (height >= hlo) & (height <= hhi)
    if kind == 'small':
        floor &= np.roll(width, -1, axis=2) <= 6
    if kind == 'deep':
        floor &= y <= -44
    candidates = np.array(np.nonzero(floor)).T
    if not len(candidates):
        return None
    # First spatially central supported position meeting the advertised shape;
    # no stone/air is changed, and no random search maximizes visual appeal.
    order = np.argsort(((candidates[:, :2] - meta['n']/2)**2).sum(axis=1), kind='stable')
    for z, x, yy in candidates[order[:2048]]:
        directions = [(dz, dx) for dz in [-1, 0, 1] for dx in [-1, 0, 1] if dz or dx]
        def visibility(d):
            dz, dx = d
            for distance in range(1, 33):
                zz, xx = z + dz*distance, x + dx*distance
                if not (0 <= zz < meta['n'] and 0 <= xx < meta['n']) or not air[zz, xx, yy+1]:
                    return distance-1
            return 32
        dz, dx = max(directions, key=visibility)
        if visibility((dz, dx)) < 8:
            continue
        return dict(seed=int(meta['seed']), eye=[meta['x']+int(x)+.5, int(yy)+meta['min_y']+1.62, meta['z']+int(z)+.5],
                    direction=[dx, .65 if kind == 'vertical' else .04, dz], width=int(width[z,x,yy]),
                    headroom=int(height[z,x,yy]), sight_blocks=visibility((dz,dx)), volume=path.name)
    return None


def entrance(path):
    cells, surface, meta = load(path)
    air = cells == 1
    y = np.arange(cells.shape[2])[None, None, :] + meta['min_y']
    depth = surface[:, :, None] - y
    floor = air & (depth >= -1) & (y >= -60)
    floor[:, :, -1] = False
    floor[:, :, 1:] &= cells[:, :, :-1] == 0
    floor[:, :, :-1] &= air[:, :, 1:]
    floor[:, :, 0] = False
    g, coords = graph(floor, air)
    if not len(coords):
        return None
    _, labels = csgraph.connected_components(g, directed=False)
    ds = depth[tuple(coords.T)]
    eligible = np.zeros(labels.max()+1, bool)
    eligible[np.unique(labels[ds >= 24])] = True
    eligible &= np.bincount(labels[ds >= 8], minlength=len(eligible)) >= 32
    a, b = g.nonzero()
    crossings = (ds[a] <= 1) & (ds[b] >= 2) & eligible[labels[a]]
    pairs = list(zip(a[crossings], b[crossings]))
    pairs.sort(key=lambda pair: ((coords[pair[0], :2]-meta['n']/2)**2).sum())
    for source, target in pairs:
        z, x, yy = coords[source]
        dz, dx, dy = coords[target]-coords[source]
        if not (10 <= z < meta['n']-10 and 10 <= x < meta['n']-10):
            continue
        # View the mouth from another supported surface floor. Looking from the
        # exact crossing edge can face the roof of a descending passage instead
        # of showing its opening. Keep an actual air ray to the interior edge.
        delta = coords[:, :2] - coords[source, :2]
        distance2 = (delta*delta).sum(axis=1)
        exterior = (labels == labels[source]) & (ds <= 1) & (distance2 >= 4) & (distance2 <= 144)
        exterior &= coords[:, 2] >= yy-1
        candidates = np.flatnonzero(exterior)
        score = abs(distance2[candidates]-36) + np.maximum(0, yy+2-coords[candidates, 2])*4
        for camera in candidates[np.argsort(score, kind='stable')[:128]]:
            start = coords[camera].astype(float) + [.5,.5,1.62]
            end = coords[target].astype(float) + [.5,.5,1.62]
            steps = max(2, int(np.linalg.norm(end-start)*4)+1)
            ray = np.floor(start + np.linspace(0,1,steps)[:,None]*(end-start)).astype(int)
            if not ((ray >= 0) & (ray < cells.shape)).all() or not air[tuple(ray.T)].all():
                continue
            zz, xx, yyy = start
            direction = end-start
            return dict(seed=int(meta['seed']), eye=[meta['x']+float(xx), meta['min_y']+float(yyy), meta['z']+float(zz)],
                        direction=direction[[1,2,0]].tolist(), volume=path.name,
                        portal=[meta['x']+int(x),meta['min_y']+int(yy),meta['z']+int(z)],
                        selection='supported exterior floor with clear air ray to a portal in a floor component reaching >=24 blocks geological depth')
    return None


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--cohort', required=True, type=Path)
    p.add_argument('--validation', required=True, type=Path)
    p.add_argument('--output', required=True, type=Path)
    p.add_argument('--executable', type=Path)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=True)
    views = []
    for kind, directory, ids in [
        ('small', a.cohort, ['s00-0', 's00-1']),
        ('medium', a.cohort, ['s01-0', 's01-1']),
        ('cavern', a.cohort, ['s01-0', 's01-6', 's02-1']),
        ('network', a.cohort, ['s03-3', 's03-0']),
        ('deep', a.validation, ['v02-0', 'v02-1', 'v00-0', 'v01-0']),
        ('vertical', a.cohort, ['s02-1', 's01-0', 's01-6']),
    ]:
        for site in ids:
            scene = interior(directory/(site+'.rcv'), kind)
            if scene:
                views.append(dict(name=kind, kind=kind, **scene))
                break
        else:
            raise RuntimeError(f'No qualifying {kind} camera in the predetermined bounded inspection sites')
    for kind, sites in [('plains-entrance', ['v00-4','v00-5','v01-4','v01-5','v02-4','v02-5','v03-4','v03-5']),
                        ('mountain-entrance', ['v00-6','v00-7','v01-6','v01-7','v02-6','v02-7','v03-6','v03-7'])]:
        for site in sites:
            scene = entrance(a.validation/(site+'.rcv'))
            if scene:
                views.append(dict(name=kind, kind=kind, **scene))
                break
        else:
            raise RuntimeError(f'No qualifying {kind} in fixed validation sites')
    (a.output/'selected.json').write_text(json.dumps(views, indent=2)+'\n')
    if not a.executable:
        return
    binary = a.executable.resolve()
    for view in views:
        modes = [(15, True)] if 'entrance' not in view['kind'] else [(15, False)]
        if view['kind'] in ['small', 'cavern']:
            # Matched pose only if the old geometry also contains that eye cell.
            cells, _, meta = load(a.cohort.parent/'v14'/view['volume'])
            x, y, z = view['eye']
            if cells[int(z//1)-meta['z'], int(x//1)-meta['x'], int(y//1)-meta['min_y']] == 1:
                modes.append((14, True))
        if view['kind'] == 'medium':
            modes.append((15, False))
        for version, diagnostic in modes:
            label = f"{view['name']}-v{version}-"+('diagnostic' if diagnostic else 'ordinary')
            dest = a.output/(label+'.png')
            if dest.exists():
                continue
            ppm = a.output/(label+'.ppm')
            x, _, z = view['eye']
            cmd = [str(binary), str(view['seed']), str(int(x)), str(int(z)), '0', f'--generator={version}',
                   f'--capture={ppm.resolve()}', '--capture-frames=32', '--radius=6',
                   '--eye='+','.join(map(str,view['eye'])), '--direction='+','.join(map(str,view['direction']))]
            if diagnostic:
                cmd.append('--diagnostic-light')
            else:
                cmd.extend(['--cave-torches', '--cave-torch-limit=3'])
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=240)
            (a.output/(label+'.log')).write_text(result.stdout+result.stderr)
            result.check_returncode()
            magic, size, maximum, rgb = ppm.read_bytes().split(b'\n',3)
            assert magic == b'P6' and maximum == b'255'
            w, h = map(int,size.split())
            assert len(rgb) == w*h*3 and len(set(rgb)) > 16
            dest.write_bytes(png(w,h,b''.join(rgb[i:i+3]+b'\xff' for i in range(0,len(rgb),3)),compression=9))
            ppm.unlink()
            (a.output/(label+'.json')).write_text(json.dumps(dict(**view, generator=version, diagnostic_light=diagnostic,
                supported_floor_torches=0 if diagnostic else 3, inspection_radius=6, geometry_untouched=True, command=cmd,
                executable_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                screenshot_sha256=hashlib.sha256(dest.read_bytes()).hexdigest()),indent=2)+'\n')
            print(label, flush=True)


if __name__ == '__main__':
    main()
