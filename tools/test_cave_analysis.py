#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Independent tiny-voxel checks for the analysis graph, not worldgen snapshots."""
import unittest
from collections import deque

import numpy as np
from scipy.sparse import csgraph
from analyze_cave_volumes import graph, routes, run_lengths, summarize, surface_walks


def oracle(mask, air=None):
    nodes = set(zip(*np.nonzero(mask)))
    result = []
    while nodes:
        todo = deque([nodes.pop()])
        component = set(todo)
        while todo:
            z, x, y = todo.popleft()
            moves = [(0,0,1),(0,0,-1),(1,0,0),(-1,0,0),(0,1,0),(0,-1,0)] if air is None else [
                (dz,dx,dy) for dz,dx in [(0,1),(0,-1),(1,0),(-1,0)] for dy in [-1,0,1]]
            for dz, dx, dy in moves:
                p = (z+dz,x+dx,y+dy)
                if p not in nodes:
                    continue
                if air is not None and dy:
                    q = (z,x,y+2) if dy > 0 else (p[0],p[1],p[2]+2)
                    if q[2] >= air.shape[2] or not air[q]:
                        continue
                nodes.remove(p)
                component.add(p)
                todo.append(p)
        result.append(component)
    return sorted(map(len,result))


class AnalysisTests(unittest.TestCase):
    def test_surface_routes_walk_and_count_blocked_endpoints(self):
        air=np.zeros((32,32,16),bool)
        air[:,:,6:]=True
        standing=np.zeros_like(air)
        standing[:,:,6]=True
        depth=np.broadcast_to(5-np.arange(16),(32,32,16))
        portals=np.zeros_like(air)
        portals[8,8,6]=True
        self.assertEqual(surface_walks(standing,air,depth,portals),(1,60,0))
        standing[:,16,:]=False
        air[:,16,:]=False
        self.assertEqual(surface_walks(standing,air,depth,portals),(1,30,2))

    def test_sparse_graph_against_independent_flood_fill(self):
        rng = np.random.default_rng(9321)
        for _ in range(30):
            air = rng.random((7,8,9)) < .55
            floors = air.copy()
            floors[:,:,0] = floors[:,:,-1] = False
            floors[:,:,1:] &= ~air[:,:,:-1]
            floors[:,:,:-1] &= air[:,:,1:]
            for mask, clearance in [(air,None),(floors,air)]:
                g,_ = graph(mask,clearance)
                _, labels = csgraph.connected_components(g,directed=False)
                self.assertEqual(sorted(np.bincount(labels).tolist()),oracle(mask,clearance))

    def test_clear_straight_route_and_obstructed_step(self):
        air = np.zeros((4,12,10),bool)
        air[1,1:11,5:7] = True
        mask = np.zeros_like(air)
        mask[1,1:11,5] = True
        _,counts,length = routes(graph(mask,air)[0])
        self.assertEqual(counts.tolist(),[10])
        self.assertEqual(length,9)
        air[1,10,5] = False
        air[1,10,7] = True
        mask[1,10,5] = False
        mask[1,10,6] = True
        self.assertEqual(sorted(routes(graph(mask,air)[0])[1].tolist()),[1,9])
        air[1,9,7] = True
        self.assertEqual(routes(graph(mask,air)[0])[1].tolist(),[10])

    def test_run_lengths_and_fluid_geometry(self):
        cells = np.zeros((8,12,32),np.uint8)
        cells[2:6,1:11,8:12] = 1
        cells[2:6,1:11,8] = 2
        mask = (cells==1)|(cells==2)
        self.assertTrue((run_lengths(mask,0)[mask]==4).all())
        self.assertTrue((run_lengths(mask,1)[mask]==10).all())
        self.assertTrue((run_lengths(mask,2)[mask]==4).all())
        result = summarize(cells,np.full((8,12),25),dict(min_y=0))
        self.assertEqual(result['cavity'],160)
        self.assertEqual(result['fluid_geometry'],40)
        self.assertEqual(result['dry_air'],120)
        self.assertEqual(result['standing_nodes'],0)


if __name__=='__main__':
    unittest.main()
