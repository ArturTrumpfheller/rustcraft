//! One embedded original atlas: no async loading, cwd dependence, or PNG decoder.
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use voxel_core::texture::ATLAS_SIZE;

const PIXELS: &[u8; 128 * 64 * 4] = include_bytes!("../../assets/terrain/atlas.rgba");

pub(crate) fn atlas_image() -> Image {
    let mut image = Image::new(
        Extent3d {
            width: ATLAS_SIZE[0],
            height: ATLAS_SIZE[1],
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        PIXELS.to_vec(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        lod_max_clamp: 0.0,
        ..ImageSamplerDescriptor::nearest()
    });
    image
}
