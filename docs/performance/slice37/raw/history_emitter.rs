// SPDX-License-Identifier: MIT OR Apache-2.0
// Copy into exact Slice36 crates/voxel_core/examples/slice37_history.rs.
use voxel_core::{ChunkPos, LocalPos, worldgen::{Generator,GenerationConfig}};
fn main(){
 println!("version,seed,cx,cz,crc,biome");
 for version in 1..=11 { for seed in [0,42,2026,u64::MAX] { for c in [[-1,0],if version>=3 {[-1511,1250]} else {[1,-1]}] {
 let g=Generator::new(GenerationConfig{seed,version}).unwrap();let s=g.sampler(c);let mut crc=crc32fast::Hasher::new();
 for y in g.world_height().chunks(){let cp=ChunkPos([c[0],y,c[1]]);let ch=s.chunk(cp).unwrap();for i in 0..4096 {let l=LocalPos::new([i%16,i/256,i/16%16]).unwrap();crc.update(&(ch.get(l) as u16).to_le_bytes());}}
 println!("{version},{seed},{},{},{},{}",c[0],c[1],crc.finalize(),g.biome(c[0]*16+8,c[1]*16+8).map_or(255,|b|b as u8));
 }}}
}
