//! Write a disposable legacy snapshot for renderer comparison, never a real save.
use voxel_core::{Block, BlockPos, World};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("pass an unused output directory")?;
    let path = std::path::PathBuf::from(path);
    if path.exists() {
        return Err("output directory already exists; refusing overwrite".into());
    }
    let mut w = World::default();
    for x in -20..20 {
        for z in -16..24 {
            w.set_block(
                BlockPos([x, 0, z]),
                if x < 6 { Block::Grass } else { Block::Stone },
            )
            .unwrap();
        }
    }
    // 24x8 stone wall crosses the negative/positive X chunk seam.
    for x in -12..12 {
        for y in 1..9 {
            w.set_block(BlockPos([x, y, -6]), Block::Stone).unwrap();
        }
    }
    // Walk-in dark chamber, a skylight aperture, original grass and wood faces.
    for x in -4..5 {
        for z in -5..5 {
            if [x, z] != [0, 0] {
                w.set_block(BlockPos([x, 6, z]), Block::Stone).unwrap();
            }
            for y in 1..6 {
                if x == -4 || x == 4 {
                    w.set_block(BlockPos([x, y, z]), Block::Stone).unwrap();
                }
            }
        }
    }
    for x in [-7, 7] {
        for y in 1..6 {
            w.set_block(BlockPos([x, y, 1]), Block::Wood).unwrap();
        }
    }
    for x in 6..10 {
        for z in -1..3 {
            w.set_block(BlockPos([x, 6, z]), Block::Leaves).unwrap();
        }
    }
    std::fs::create_dir_all(&path)?;
    std::fs::write(path.join("world.rcw"), voxel_core::persistence::encode(&w)?)?;
    println!(
        "Import the legacy world from {} using the title's world list.",
        path.display()
    );
    Ok(())
}
