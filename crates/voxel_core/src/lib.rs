//! Engine-independent voxel storage, coordinates and surface generation.

mod chunk;
pub mod interaction;
pub mod inventory;
mod mesh;
pub mod persistence;
pub mod physics;
pub mod raycast;
pub mod regions;
pub mod streaming;
pub mod texture;
mod world;
pub mod worldgen;

pub use chunk::{Block, BlockPos, CHUNK_SIZE, Chunk, ChunkPos, LocalPos};
pub use mesh::{ChunkMesh, mesh_chunk};
pub use world::{BlockChange, EDIT_MAX, EDIT_MIN, StreamIdentity, World, demo_world};
