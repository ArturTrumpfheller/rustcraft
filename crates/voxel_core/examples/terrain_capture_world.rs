// SPDX-License-Identifier: MIT OR Apache-2.0
//! Prepare an isolated new-world catalog for genuine client capture/reload.
//! terrain_capture_world ABSOLUTE_EMPTY_DIRECTORY SEED
use std::path::PathBuf;
use voxel_core::persistence::catalog::WorldCatalog;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 2, "ABSOLUTE_EMPTY_DIRECTORY SEED");
    let path = PathBuf::from(&args[0]);
    assert!(path.is_absolute());
    assert!(
        !path.exists() || std::fs::read_dir(&path).unwrap().next().is_none(),
        "capture directory must be absent or empty"
    );
    let catalog = WorldCatalog::new(path).unwrap();
    let id = catalog
        .create("Slice 36 capture", args[1].parse().unwrap())
        .unwrap();
    println!(
        "Created new capture world {id} with default generator {}",
        voxel_core::worldgen::CURRENT_LOCAL_GENERATOR
    );
}
