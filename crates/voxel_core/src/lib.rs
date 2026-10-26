//! Engine-independent voxel storage, coordinates, fluids and surface generation.

mod chunk;
pub mod drops;
pub mod fluid;
pub mod interaction;
pub mod inventory;
pub mod item_art;
pub mod lighting;
mod mesh;
pub mod persistence;
pub mod physics;
pub mod raycast;
pub mod regions;
pub mod shape;
pub mod streaming;
pub mod texture;
pub mod time;
pub mod water_mesh;
mod world;
pub mod worldgen;

pub use chunk::{Block, BlockPos, CHUNK_SIZE, Chunk, ChunkPos, LocalPos};
pub use mesh::{ChunkMesh, mesh_chunk, mesh_chunk_lit, torch_mesh};
pub use world::{BlockChange, EDIT_MAX, EDIT_MIN, EditBatch, StreamIdentity, World, demo_world};

#[cfg(test)]
mod torch_tests;

#[cfg(test)]
mod support_tests;

#[cfg(test)]
mod bucket_tests;
