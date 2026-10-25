//! Thin Bevy adapter for core selection boxes. Rebuilt only when shape changes.
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use voxel_core::Block;

pub(crate) fn outline_mesh(block: Block) -> Mesh {
    let mut corners = Vec::new();
    let mut indices = Vec::new();
    let frame = block.shape_transform();
    for bounds in block.selection_boxes() {
        let start = corners.len() as u32;
        // Small local expansion prevents coplanar flicker, including tilted faces.
        let mut bounds = *bounds;
        for a in 0..3 {
            bounds.min[a] -= 0.002;
            bounds.max[a] += 0.002;
        }
        corners.extend(bounds.corners().map(|p| frame.point(p).map(|v| v as f32)));
        for i in 0..8 {
            for bit in [1, 2, 4] {
                if i & bit == 0 {
                    indices.extend([start + i, start + (i | bit)]);
                }
            }
        }
    }
    let count = corners.len();
    Mesh::new(
        PrimitiveTopology::LineList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, corners)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; count])
    .with_inserted_indices(Indices::U32(indices))
}
