#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Paired, untouched Bevy geometry; fixed poses selected from measured voxels.
The small inspection set illustrates mechanisms, not entrance prevalence.
"""
import argparse, csv, hashlib, json, subprocess
from pathlib import Path
import numpy as np
from analyze_cave_volumes import load
from analyze_cave_exposure import line_clear
from scipy import ndimage
from generate_textures import png


def surface_camera(row, mouth, old, new, base, meta):
    exterior = base == 1
    floors = exterior.copy()
    floors[:, :, 0] = floors[:, :, -1] = False
    floors[:, :, 1:] &= base[:, :, :-1] == 0
    floors[:, :, :-1] &= exterior[:, :, 1:]
    coords = np.array(np.nonzero(floors)).T
    center = np.array(mouth['center'])
    distance = ((coords[:, :2] - center[:2])**2).sum(axis=1)
    candidates = np.flatnonzero((distance >= 16) & (distance <= 256))
    score = abs(distance[candidates]-64) + abs(coords[candidates, 2]-center[2])*8
    box = tuple(slice(*b) for b in mouth['box'])
    points = np.array(np.nonzero((base[box] == 0) & (old[box] == 1))).T
    points += np.array([b.start for b in box])
    if not len(points):
        return None
    interior_target = False
    if mouth['deep']:
        region = np.zeros_like(exterior)
        expanded = tuple(slice(max(0,b.start-12),min(base.shape[i],b.stop+12)) for i,b in enumerate(box))
        region[expanded] = True
        inside = np.array(np.nonzero(region & (base==0) & (old==1) & (ndimage.distance_transform_edt(base==0)>=3))).T
        if len(inside):
            points = inside
            interior_target = True
    target_center = center.copy()
    if interior_target:
        target_center[2] -= 3
    targets = points[np.argsort(((points-target_center)**2).sum(axis=1), kind='stable')[:128]]+.5
    for i in candidates[np.argsort(score, kind='stable')[:128]]:
        start = coords[i].astype(float)+[.5, .5, 1.62]
        q = tuple(np.floor(start).astype(int))
        if old[q] != 1 or new[q] != 1:
            continue
        for end in targets:
            if line_clear(old == 1, start, end):
                z, x, y = start
                return dict(seed=int(row['seed']), eye=[meta['x']+x, meta['min_y']+y, meta['z']+z],
                            direction=(end-start)[[1, 2, 0]].tolist(), volume=row['id']+'.rcv',
                            aperture=mouth, interior_air_target=interior_target, selection='supported uncaved exterior floor, clear v15 supercover ray; same air camera in both versions')
    return None


def select(root):
    rows = [json.loads(l) for l in (root/'v15-primary.jsonl').read_text().splitlines()]
    views = []
    for name, predicate in [
        ('plains', lambda m:m['terrain']=='plain'),
        ('hills', lambda m:m['terrain']=='hill'),
        ('mountains', lambda m:m['terrain']=='mountain'),
        ('large-mouth', lambda m:m['deep'] and m['faces']>=64),
    ]:
        found = None
        for row in rows:
            choices = [m for m in row['mouths'] if predicate(m) and m['visible'] and (m['two_block'] or m['penetration']>=3)]
            if not choices:
                continue
            old, _, meta = load(root/'v15-primary-final'/(row['id']+'.rcv'))
            new, _, _ = load(root/'v16-primary-final'/(row['id']+'.rcv'))
            base, _, _ = load(root/'base-primary'/(row['id']+'.rcv'))
            for mouth in choices:
                found = surface_camera(row, mouth, old, new, base, meta)
                if found:
                    break
            if found:
                break
        if not found:
            raise RuntimeError('No valid camera for '+name)
        if name == 'mountains':
            # Give steep terrain context instead of aiming a standing camera
            # at the nearest stone ledge directly in front of its face.
            target = np.array(found['eye']) + found['direction']
            eye = np.array(found['eye']) + [0.,14.,0.]
            found.update(eye=eye.tolist(),direction=(target-eye).tolist(),selection='camera14 blocks above the original supported exterior pose; identical close mountain context in both versions')
        views.append(dict(name=name, diagnostic=False, **found))
    # A bounded inspection control: the quietest lowland unconditional window
    # with no qualifying opening. This is explicitly not a prevalence sample.
    plan = {r['id']:r for r in csv.DictReader(Path('docs/performance/slice42/raw/cohort.csv').open())}
    quiet = [r for r in rows if 'unconditional' in r['stratum'] and r['entrance_openings']==0
             and 20<=int(plan[r['id']]['mean_height'])<=42 and int(plan[r['id']]['relief'])<=4]
    found_quiet = False
    for row in sorted(quiet,key=lambda r:(r['openings'],r['id'])):
        base, ground, meta = load(root/'base-primary'/(row['id']+'.rcv'))
        old, _, _ = load(root/'v15-primary-final'/(row['id']+'.rcv'))
        new, _, _ = load(root/'v16-primary-final'/(row['id']+'.rcv'))
        zz,xx = np.indices(ground.shape)
        floor = ground-meta['min_y']
        stable = (old[zz,xx,floor]==0)&(new[zz,xx,floor]==0)
        for dy in [1,2]:
            stable &= (old[zz,xx,floor+dy]==1)&(new[zz,xx,floor+dy]==1)
        stable = ndimage.minimum_filter(stable,size=5,mode='constant')
        stable[:8]=stable[-8:]=False
        stable[:,:8]=stable[:,-8:]=False
        candidates=np.array(np.nonzero(stable)).T
        if len(candidates):
            z,x=candidates[np.argmin(((candidates-64)**2).sum(axis=1))]
            x,z=int(x),int(z)
            found_quiet=True
            break
    if not found_quiet:
        raise RuntimeError('No supported quiet-landscape camera in bounded control sites')
    views.append(dict(name='ordinary', diagnostic=False, seed=int(row['seed']),
        eye=[meta['x']+x+.5, float(ground[z,x]+2.62), meta['z']+z+.5],
        direction=[1., -.04, 1.], volume=row['id']+'.rcv', selection='quietest bounded unconditional flat lowland window with no qualifying surface opening; supported unchanged5x5 exterior patch nearest the center'))
    original = json.loads(Path('docs/performance/slice42/raw/captures/selected.json').read_text())
    for name in ['network', 'cavern']:
        view = next(v for v in original if v['name']==name)
        views.append(dict(**view, diagnostic=True, selection='unchanged original Slice42 interior pose'))
    return views


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--executable', type=Path)
    args=parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    selected=args.output/'selected.json'
    if selected.exists():
        views=json.loads(selected.read_text())
    else:
        views=select(args.work)
        selected.write_text(json.dumps(views, indent=2)+'\n')
    if not args.executable:
        return
    binary=args.executable.resolve()
    for view in views:
        for version in [15, 16]:
            label=f"{view['name']}-v{version}"
            dest=args.output/(label+'.png')
            if dest.exists():
                continue
            ppm=args.output/(label+'.ppm')
            x, _, z=view['eye']
            cmd=[str(binary), str(view['seed']), str(int(x)), str(int(z)), '0', f'--generator={version}',
                 f'--capture={ppm.resolve()}', '--capture-frames=32', '--radius=6',
                 '--eye='+','.join(map(str,view['eye'])), '--direction='+','.join(map(str,view['direction']))]
            if view['diagnostic']:
                cmd.append('--diagnostic-light')
            result=subprocess.run(cmd, capture_output=True, text=True, timeout=240)
            (args.output/(label+'.log')).write_text(result.stdout+result.stderr)
            result.check_returncode()
            magic, size, maximum, rgb=ppm.read_bytes().split(b'\n', 3)
            assert magic==b'P6' and maximum==b'255' and len(set(rgb))>16
            width, height=map(int, size.split())
            assert len(rgb)==width*height*3
            dest.write_bytes(png(width,height,b''.join(rgb[i:i+3]+b'\xff' for i in range(0,len(rgb),3)),compression=9))
            ppm.unlink()
            (args.output/(label+'.json')).write_text(json.dumps(dict(**view,generator=version,
                command=cmd, inspection_radius=6, geometry_untouched=True, placed_lights=0,
                executable_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                screenshot_sha256=hashlib.sha256(dest.read_bytes()).hexdigest()),indent=2)+'\n')
            print(label, flush=True)

if __name__=='__main__':
    main()
