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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::image::{ImageAddressMode, ImageFilterMode};

    #[test]
    fn embedded_atlas_is_srgb_opaque_nearest_lod_zero_without_anisotropy() {
        let image = atlas_image();
        let t = &image.texture_descriptor;
        assert_eq!(
            [t.size.width, t.size.height, t.size.depth_or_array_layers],
            [128, 64, 1]
        );
        assert_eq!(t.format, TextureFormat::Rgba8UnormSrgb);
        assert_eq!(t.mip_level_count, 1);
        assert_eq!(image.data.as_deref(), Some(PIXELS.as_slice()));
        assert!(PIXELS.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
        let ImageSampler::Descriptor(s) = image.sampler else {
            panic!("explicit sampler required")
        };
        assert_eq!(s.mag_filter, ImageFilterMode::Nearest);
        assert_eq!(s.min_filter, ImageFilterMode::Nearest);
        assert_eq!(s.mipmap_filter, ImageFilterMode::Nearest);
        assert_eq!(s.lod_min_clamp, 0.);
        assert_eq!(s.lod_max_clamp, 0.);
        assert_eq!(s.anisotropy_clamp, 1);
        assert_eq!(s.address_mode_u, ImageAddressMode::ClampToEdge);
        assert_eq!(s.address_mode_v, ImageAddressMode::ClampToEdge);
    }

    #[test]
    fn every_slot_extrudes_exact_edge_pixels_and_committed_tiles_are_distinct() {
        use std::collections::BTreeSet;
        let pixel = |x: usize, y: usize| &PIXELS[(y * 128 + x) * 4..(y * 128 + x + 1) * 4];
        assert_eq!(pixel(8, 8), [129, 156, 77, 255]); // editable grass-top row 0, symbol 2
        let mut unique = BTreeSet::new();
        for i in 0..7 {
            let ox = i % 4 * 32;
            let oy = i / 4 * 32;
            let mut tile = Vec::new();
            let mut palette = BTreeSet::new();
            for y in 0..32 {
                for x in 0..32 {
                    assert_eq!(
                        pixel(ox + x, oy + y),
                        pixel(ox + x.clamp(8, 23), oy + y.clamp(8, 23))
                    );
                    if (8..24).contains(&x) && (8..24).contains(&y) {
                        tile.extend_from_slice(pixel(ox + x, oy + y));
                        palette.insert(pixel(ox + x, oy + y));
                    }
                }
            }
            assert!(palette.len() >= 4);
            assert!(unique.insert(tile));
        }
    }
}
