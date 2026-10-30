//! Two crossed, chunk-batched cutout quads per plant. No sorting or cube faces.
use crate::{Block, BlockPos, ChunkMesh};

/// The same vertex contract as terrain; vectors remain unallocated without plants.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlantMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub atlas_uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}
impl PlantMesh {
    pub fn into_surface(self) -> ChunkMesh {
        ChunkMesh {
            positions: self.positions,
            normals: self.normals,
            uvs: self.uvs,
            atlas_uvs: self.atlas_uvs,
            colors: self.colors,
            indices: self.indices,
            ..Default::default()
        }
    }
    pub(crate) fn append(
        &mut self,
        block: Block,
        local: [f32; 3],
        world: BlockPos,
        light: &crate::lighting::Skylight,
    ) {
        let tile = crate::texture::face_tile(block, [0, 0, 1])
            .unwrap()
            .uv([0, 0]);
        let h = block.plant_height();
        let color = [
            crate::lighting::face_color(light.level(world), [0, 1, 0])[0],
            crate::lighting::face_color(light.block_level(world), [0, 1, 0])[0],
            f32::from(light.level(world)) / 15.,
            1.,
        ];
        for (ends, normal) in [
            (
                [[0.125, 0.125], [0.875, 0.875]],
                [-0.70710677, 0., 0.70710677],
            ),
            (
                [[0.875, 0.125], [0.125, 0.875]],
                [-0.70710677, 0., -0.70710677],
            ),
        ] {
            let start = self.positions.len() as u32;
            for [x, y, z] in [
                [ends[0][0], 0., ends[0][1]],
                [ends[1][0], 0., ends[1][1]],
                [ends[1][0], h, ends[1][1]],
                [ends[0][0], h, ends[0][1]],
            ] {
                self.positions
                    .push([local[0] + x, local[1] + y, local[2] + z]);
            }
            self.normals.extend([normal; 4]);
            self.uvs.extend([[0., 1.], [1., 1.], [1., 0.], [0., 0.]]);
            self.atlas_uvs.extend([tile; 4]);
            self.colors.extend([color; 4]);
            self.indices
                .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
        }
    }
}
