# SPDX-License-Identifier: MIT OR Apache-2.0
import unittest
import numpy as np
from analyze_cave_exposure import exposure,line_clear
class ExposureTests(unittest.TestCase):
    def scene(self):
        base=np.zeros((64,64,64),np.uint8);base[:,:,40:]=1
        return base,np.full((64,64),39),dict(min_y=0)
    def test_supercover_ray_rejects_corner_only_sight(self):
        air=np.ones((5,5,5),bool)
        self.assertTrue(line_clear(air,[.5,.5,.5],[4.5,4.5,4.5]))
        air[1,0,0]=False
        self.assertFalse(line_clear(air,[.5,.5,.5],[4.5,4.5,4.5]))
    def test_surface_scuff_is_separate_from_two_block_opening(self):
        base,h,m=self.scene();cells=base.copy();cells[20:25,12:17,39]=1
        r=exposure(cells,base,h,m)
        self.assertEqual(r['openings'],1)
        self.assertEqual(r['two_block_openings'],0)
        self.assertEqual(r['penetrating_openings'],0)
        self.assertEqual(r['surface_scars'],1)
    def test_two_physical_mouths_can_share_one_cavity_without_walkable_access(self):
        base,h,m=self.scene();cells=base.copy()
        for x in [20,44]:cells[29:32,x-1:x+2,9:40]=1
        cells[29:32,19:46,9:13]=1
        r=exposure(cells,base,h,m)
        self.assertEqual(r['two_block_openings'],0)
        self.assertEqual(r['entrance_openings'],2)
        self.assertEqual(r['visible_entrance_openings'],2)
        self.assertEqual(r['exposed_systems'],1)
        self.assertEqual(r['systems_with_multiple_openings'],1)
        self.assertEqual(r['navigable_openings'],0)
    def test_real_supported_stair_to_deep_room(self):
        base,h,m=self.scene();cells=base.copy()
        for x in range(12,44):
            floor=39-(x-12);cells[29:32,x,floor+1:floor+4]=1
        cells[24:37,38:52,9:13]=1
        r=exposure(cells,base,h,m)
        self.assertEqual(r['two_block_openings'],1)
        self.assertEqual(r['two_block_navigable_openings'],1)
        self.assertEqual(r['two_block_deep_openings'],1)
    def test_shallow_hole_is_not_deep_access_via_a_different_surface_entrance(self):
        base,h,m=self.scene();cells=base.copy()
        for x in range(12,44):
            floor=39-(x-12);cells[29:32,x,floor+1:floor+4]=1
        cells[24:37,38:52,9:13]=1
        cells[10:14,16:20,38:40]=1
        r=exposure(cells,base,h,m)
        self.assertEqual(r['entrance_openings'],2)
        self.assertEqual(r['navigable_entrance_openings'],1)
        self.assertFalse(any(v['navigable'] for v in r['mouths'] if not v['deep']))
    def test_native_reference_height_offset_changes_terrain_label_only(self):
        base=np.zeros((64,64,128),np.uint8);base[:,:,101:]=1
        ground=np.full((64,64),100);cells=base.copy();cells[20:25,12:17,99:101]=1
        native=exposure(cells,base,ground,dict(min_y=0))
        normalized=exposure(cells,base,ground,dict(min_y=0),surface_offset=61)
        self.assertEqual(native['mouths'][0]['terrain'],'mountain')
        self.assertEqual(normalized['mouths'][0]['terrain'],'plain')
        self.assertEqual(native['entrance_openings'],normalized['entrance_openings'])
        self.assertEqual(native['deep_cavity_cells'],normalized['deep_cavity_cells'])
    def test_deep_changes_detect_even_new_isolated_air(self):
        base,h,m=self.scene();cells=base.copy();cells[32,32,8]=1
        r=exposure(cells,base,h,m,previous=base)
        self.assertEqual(r['deep_changed_cells'],1)
        self.assertEqual(r['new_cavity_cells'],1)
if __name__=='__main__':unittest.main()
