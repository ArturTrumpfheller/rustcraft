#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Common final-occupancy cave analysis; no Minecraft implementation or data.

Analysis-only dependencies: numpy, scipy, matplotlib (plots). RCV1 is little
endian: magic, i16(n,height,min_y), u64(seed), i32(x,z), i16 surface[n,n],
u8 cells[n,n,height] in Z,X,Y order. Codes: 0 full solid, 1 dry Air, 2 water,
3 lava, 4 other obstacles. Surface excludes trees/decorations and liquids.
"""
import argparse
import csv
import hashlib
import json
import struct
from pathlib import Path

import numpy as np
from scipy import ndimage, sparse
from scipy.sparse import csgraph


def load(path):
    data = path.read_bytes()
    assert data[:4] == b'RCV1', path
    n, height, low, seed, x, z = struct.unpack_from('<hhhQii', data, 4)
    assert 0 < n <= 256 and 0 < height <= 512
    offset = 26
    assert len(data) == offset + n*n*2 + n*n*height, path
    surface = np.frombuffer(data, '<i2', n*n, offset).reshape(n, n).astype(np.int32)
    cells = np.frombuffer(data, 'u1', n*n*height, offset+n*n*2).reshape(n, n, height)
    assert np.isin(cells, [0, 1, 2, 3, 4]).all()
    return cells, surface, dict(n=n, height=height, min_y=low, seed=str(seed), x=x, z=z,
                               sha256=hashlib.sha256(data).hexdigest())


def graph(mask, air=None):
    """6-neighbor geometry, or supported floor moves with 2-block clearance.

    Floors permit level moves and one-block jumps/drops only. For either step
    direction the lower column needs the extra head voxel during the move.
    Fluids and non-cube obstacles never become support or dry clearance.
    """
    coords = np.array(np.nonzero(mask)).T
    count = len(coords)
    ids = np.full(mask.shape, -1, np.int32)
    ids[tuple(coords.T)] = np.arange(count, dtype=np.int32)
    starts, ends = [], []
    moves = [(0, 0, 1), (0, 1, 0), (1, 0, 0)] if air is None else [
        (dz, dx, dy) for dz, dx in [(0, 1), (1, 0)] for dy in [-1, 0, 1]]
    for move in moves:
        target = coords + move
        valid = ((target >= 0) & (target < mask.shape)).all(axis=1)
        src = np.flatnonzero(valid)
        target = target[valid]
        dst = ids[tuple(target.T)]
        valid = dst >= 0
        if air is not None and move[2]:
            lower = coords[src].copy() if move[2] > 0 else target.copy()
            lower[:, 2] += 2
            clearance = lower[:, 2] < mask.shape[2]
            q = lower[clearance]
            clearance[clearance] = air[tuple(q.T)]
            valid &= clearance
        starts.append(src[valid])
        ends.append(dst[valid])
    a, b = np.concatenate(starts), np.concatenate(ends)
    g = sparse.csr_matrix((np.ones(len(a), np.uint8), (a, b)), shape=(count, count))
    return g, coords


def routes(g):
    if not g.shape[0]:
        return np.array([], np.int32), np.array([], np.int64), 0
    _, labels = csgraph.connected_components(g, directed=False)
    counts = np.bincount(labels)
    start = np.flatnonzero(labels == counts.argmax())[0]
    d = csgraph.shortest_path(g, directed=False, unweighted=True, indices=start)
    start = np.where(np.isfinite(d), d, -1).argmax()
    d = csgraph.shortest_path(g, directed=False, unweighted=True, indices=start)
    return labels, counts, int(d[np.isfinite(d)].max())


def run_lengths(mask, axis):
    shape = [1]*3
    shape[axis] = mask.shape[axis]
    p = np.arange(mask.shape[axis], dtype=np.int16).reshape(shape)
    before = np.maximum.accumulate(np.where(mask, -1, p), axis=axis)
    rev = [slice(None)]*3
    rev[axis] = slice(None, None, -1)
    after = np.minimum.accumulate(np.where(mask, mask.shape[axis], p)[tuple(rev)], axis=axis)[tuple(rev)]
    return after-before-1


def surface_walks(standing, dry, depth, portals):
    """Four predetermined surface waypoints, shortest supported ground paths.

    Paths may detour around cliffs/obstacles; disconnected endpoints are counted
    and omitted from walked distance. This is a conservative floor-graph survey,
    not a recorded gameplay controller trajectory. Window clipping can block it.
    """
    g, coords = graph(standing & (depth <= 1), dry)
    route = np.zeros_like(standing)
    distance, blocked = 0, 0
    if not len(coords):
        return 0, 0, 4
    n = standing.shape[0]
    targets = []
    for lane in [n//4,n*3//4]:
        targets.extend([((lane,8),(lane,n-9)),((8,lane),(n-9,lane))])
    for start, end in targets:
        d0 = ((coords[:,:2]-start)**2).sum(axis=1)
        d1 = ((coords[:,:2]-end)**2).sum(axis=1)
        if min(d0) > 64 or min(d1) > 64:
            blocked += 1
            continue
        source, target = int(d0.argmin()), int(d1.argmin())
        lengths, previous = csgraph.shortest_path(g,directed=False,unweighted=True,
                                                 indices=source,return_predecessors=True)
        if not np.isfinite(lengths[target]):
            blocked += 1
            continue
        distance += int(lengths[target])
        while target != source:
            route[tuple(coords[target])] = True
            target = int(previous[target])
        route[tuple(coords[source])] = True
    near = ndimage.maximum_filter(route,size=7,mode='constant',cval=0)
    portal_labels, _ = ndimage.label(portals,structure=np.ones((3,3,3),bool))
    encountered = len(np.unique(portal_labels[portals & near]))
    return encountered, distance, blocked


def summarize(cells, surface, meta):
    y = np.arange(cells.shape[2])[None, None, :] + meta['min_y']
    depth = surface[:, :, None]-y
    # Match geological volumes: omit foundation and the first eight roof cells.
    under = (y >= meta['min_y']+4) & (depth >= 8)
    dry = cells == 1
    cavity = (dry | (cells == 2) | (cells == 3)) & under
    total = int(under.sum())
    labels, components = ndimage.label(cavity)
    counts = np.bincount(labels.ravel())[1:]
    largest = int(counts.max(initial=0))
    major = labels == (int(counts.argmax())+1) if largest else np.zeros_like(cavity)
    major_graph, major_coords = graph(major)
    _, _, air_route = routes(major_graph)
    # Include surface floors in the separate traversal graph to detect entrances.
    standing = dry & (depth >= -1) & (y >= meta['min_y']+4)
    standing[:, :, -1] = False
    standing[:, :, 1:] &= (cells[:, :, :-1] == 0)
    standing[:, :, :-1] &= dry[:, :, 1:]
    standing[:, :, 0] = False
    floors = standing & under
    walk_graph, walk_coords = graph(floors, dry)
    walk_labels, walk_counts, walk_route = routes(walk_graph)
    full_graph, full_coords = graph(standing, dry)
    if len(full_coords):
        _, full_labels = csgraph.connected_components(full_graph, directed=False)
        full_depth = depth[tuple(full_coords.T)]
        eligible = np.zeros(full_labels.max()+1, bool)
        eligible[np.unique(full_labels[full_depth >= 24])] = True
        deep_counts = np.bincount(full_labels[full_depth >= 8], minlength=len(eligible))
        eligible &= deep_counts >= 32
        a, b = full_graph.nonzero()
        crossing = (((full_depth[a] <= 1) & (full_depth[b] >= 2)) |
                    ((full_depth[b] <= 1) & (full_depth[a] >= 2))) & eligible[full_labels[a]]
        portals = np.zeros_like(standing)
        portals[tuple(full_coords[a[crossing]].T)] = True
        portals[tuple(full_coords[b[crossing]].T)] = True
        _, mouths = ndimage.label(portals, structure=np.ones((3, 3, 3), bool))
        # Fixed surface exploration transects, not a search for cave-rich paths.
        route_mask = np.zeros_like(portals)
        for lane in [cells.shape[0]//4, cells.shape[0]*3//4]:
            route_mask[max(0,lane-3):lane+4, :, :] = True
            route_mask[:, max(0,lane-3):lane+4, :] = True
        portal_labels, _ = ndimage.label(portals, structure=np.ones((3, 3, 3), bool))
        route_mouths = len(np.unique(portal_labels[portals & route_mask]))
        walk_mouths, walk_length, blocked_walks = surface_walks(standing,dry,depth,portals)
    else:
        mouths = route_mouths = 0
        walk_mouths = walk_length = 0
        blocked_walks = 4
    relative = depth / np.maximum(1, surface[:, :, None]-(meta['min_y']+4)+1)
    bins = np.linspace(0, 1, 11)
    geo_depth = np.histogram(relative[under], bins)[0]
    void_depth = np.histogram(relative[cavity], bins)[0]
    dry_depth = np.histogram(relative[dry & under], bins)[0]
    fixed_bins = [8, 16, 32, 64, 96, 128, 192, 512]
    fixed_geo = np.histogram(np.broadcast_to(depth, cells.shape)[under], fixed_bins)[0]
    fixed_void = np.histogram(np.broadcast_to(depth, cells.shape)[cavity], fixed_bins)[0]
    widths = np.minimum(run_lengths(cavity, 0), run_lengths(cavity, 1))
    ceiling = run_lengths(cavity, 2)
    size_bins = [0, 2, 4, 8, 16, 32, 64, 1024]
    width_hist = np.histogram(widths[cavity], size_bins)[0]
    height_hist = np.histogram(ceiling[cavity], size_bins)[0]
    chamber = cavity & (widths >= 16) & (ceiling >= 8)
    room_labels, _ = ndimage.label(chamber)
    room_sizes = np.bincount(room_labels.ravel())[1:]
    edge_labels = np.unique(np.concatenate([labels[0].ravel(), labels[-1].ravel(),
                                          labels[:, 0].ravel(), labels[:, -1].ravel()]))
    edge_labels = edge_labels[edge_labels != 0]
    return dict(**meta, geology=total, dry_air=int((dry & under).sum()),
                fluid_geometry=int(((cells == 2) | (cells == 3))[under].sum()),
                cavity=int(cavity.sum()), cavity_fraction=float(cavity.sum()/max(total, 1)),
                mean_surface=float(surface.mean()), relief=int(np.ptp(surface)),
                components=components, largest=largest,
                component_bins=np.histogram(counts, [1,64,512,4096,32768,262144,np.inf])[0].tolist(),
                boundary_censored_components=len(edge_labels),
                largest_vertical_extent=int(np.ptp(major_coords[:, 2])+1) if largest else 0,
                geometry_geodesic_lower_bound=air_route,
                standing_nodes=int(floors.sum()), standing_components=len(walk_counts),
                largest_standing=int(walk_counts.max(initial=0)),
                standing_geodesic_lower_bound=walk_route,
                entrance_portal_clusters=int(mouths), transect_portal_clusters=route_mouths,
                transect_block_length=4*cells.shape[0],
                walked_portal_clusters=walk_mouths, surface_walk_block_length=walk_length,
                blocked_surface_walks=blocked_walks,
                normalized_geology=geo_depth.tolist(), normalized_cavity=void_depth.tolist(),
                normalized_dry_air=dry_depth.tolist(), fixed_geology=fixed_geo.tolist(),
                fixed_cavity=fixed_void.tolist(), width_bins=width_hist.tolist(),
                headroom_bins=height_hist.tolist(), broad_core_components=len(room_sizes),
                broad_core_sizes=np.histogram(room_sizes,[1,64,512,4096,32768,np.inf])[0].tolist())


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('directory', type=Path)
    p.add_argument('--plan', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    plan = list(csv.DictReader(a.plan.open()))
    a.output.parent.mkdir(parents=True, exist_ok=True)
    with a.output.open('x') as out:
        for row in plan:
            path = a.directory/(row['id']+'.rcv')
            cells, surface, meta = load(path)
            assert str(meta['seed']) == row['seed'] and meta['n'] == int(row['n'])
            result = summarize(cells, surface, meta)
            result.update(id=row['id'], stratum=row['stratum'], selection_attempt=int(row['attempt']))
            out.write(json.dumps(result, separators=(',', ':'))+'\n')
            out.flush()
            print(row['id'], result['cavity_fraction'], flush=True)


if __name__ == '__main__':
    main()
