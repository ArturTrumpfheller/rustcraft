//! One embedded original atlas: no async loading, cwd dependence, or PNG decoder.
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use voxel_core::texture::ATLAS_SIZE;

const PIXELS: &[u8; 128 * 192 * 4] = include_bytes!("../../assets/terrain/atlas.rgba");

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

/// Original 16x16 ripple tile, authored algorithmically; MIT OR Apache-2.0.
#[allow(dead_code)] // Shared module in the historical terrain-only galleries.
pub(crate) fn water_image() -> Image {
    let pixels = voxel_core::item_art::water_pixels();
    let mut image = Image::new(
        Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        lod_max_clamp: 0.,
        ..ImageSamplerDescriptor::nearest()
    });
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::image::{ImageAddressMode, ImageFilterMode};

    #[test]
    fn original_water_is_a_distinct_pixel_grid_and_both_bucket_icons_are_recognizable() {
        use voxel_core::{inventory::Item, item_art};
        let image = water_image();
        assert_eq!(image.texture_descriptor.size.width, 16);
        assert_eq!(image.texture_descriptor.size.height, 16);
        assert_eq!(image.data.as_ref().unwrap().len(), 1024);
        assert_eq!(image.data.as_ref().unwrap(), &item_art::water_pixels());
        let image_for = |item| {
            (0..16)
                .flat_map(move |y| (0..16).map(move |x| item_art::bucket_pixel(item, x, y)))
                .collect::<Vec<_>>()
        };
        let empty = image_for(Item::EmptyBucket);
        let full = image_for(Item::WaterBucket);
        assert_ne!(empty, full);
        assert_eq!(empty[0], None);
        assert_eq!(full[0], None);
        assert!(
            full.iter()
                .flatten()
                .any(|p| u16::from(p[2]) > u16::from(p[0]) * 2)
        );
        assert!(empty.iter().flatten().count() > 64);
    }

    #[test]
    fn embedded_atlas_is_srgb_cutout_nearest_lod_zero_without_anisotropy() {
        let image = atlas_image();
        let t = &image.texture_descriptor;
        assert_eq!(
            [t.size.width, t.size.height, t.size.depth_or_array_layers],
            [128, 192, 1]
        );
        assert_eq!(t.format, TextureFormat::Rgba8UnormSrgb);
        assert_eq!(t.mip_level_count, 1);
        assert_eq!(image.data.as_deref(), Some(PIXELS.as_slice()));
        assert!(
            PIXELS
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[3] == 0 || p[3] == 255)
        );
        assert!(PIXELS.as_chunks::<4>().0.iter().any(|p| p[3] == 0));
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
        for i in 0..11 {
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

#[cfg(test)]
mod vegetation_art_tests {
    use super::*;
    #[test]
    fn species_tiles_are_opaque_and_five_plant_tiles_have_binary_cutouts() {
        for i in 0..22 {
            let mut opaque = 0;
            let mut clear = 0;
            for y in 8..24 {
                for x in 8..24 {
                    let a = PIXELS[(((i / 4) * 32 + y) * 128 + (i % 4) * 32 + x) * 4 + 3];
                    match a {
                        0 => clear += 1,
                        255 => opaque += 1,
                        _ => panic!("blended sprite pixel"),
                    }
                }
            }
            if i < 17 {
                assert_eq!(opaque, 256);
            } else {
                assert!(clear > 100 && opaque > 20, "tile {i}: {opaque}/{clear}");
            }
        }
    }
}
