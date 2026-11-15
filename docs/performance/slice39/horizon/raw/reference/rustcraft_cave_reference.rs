// SPDX-License-Identifier: MIT OR Apache-2.0
// RustCraft's analysis-only adapter. Horizon itself is unmodified MIT source.
// Reads external functional registry definitions; no game assets or JARs.
use std::{collections::BTreeMap,env,fs,io::Write,path::Path,sync::Arc};
use horizon_core::{density::{DensitySampler,SamplerContext,graph::DensityCompiler},noise::DensityVolume,position::ChunkPos,random::{XoroshiroRandom,RandomSource,PositionalRandom}};
use horizon_world::{registry::BlockStateId,worldgen::{density::DensityRegistry,anchor::WorldGenerationContext,carver::{CarverCatalog,generate_carving_mask},aquifer::{AquiferConfig,AquiferSamplers,AquiferField,AquiferBlockStates,GlobalFluidPicker,NoiseBasedAquifer}}};
type Result<T> = std::result::Result<T,Box<dyn std::error::Error>>;
fn bundle(dir:&Path)->Result<BTreeMap<String,String>> {
 fn visit(dir:&Path,root:&Path,out:&mut BTreeMap<String,String>)->Result<()> {
  for f in fs::read_dir(dir)? { let p=f?.path();if p.is_dir(){visit(&p,root,out)?;} else if p.extension().is_some_and(|v|v=="json") {out.insert(format!("minecraft:{}",p.strip_prefix(root)?.with_extension("").to_string_lossy().replace('\\',"/")),fs::read_to_string(p)?);}}
  Ok(())
 }
 let mut out=BTreeMap::new();visit(dir,dir,&mut out)?;Ok(out)
}
#[derive(Clone,Copy)] struct States;
impl AquiferBlockStates for States {
 fn air(&self)->BlockStateId{BlockStateId(1)}
 fn lava(&self)->BlockStateId{BlockStateId(3)}
 fn is_air(&self,s:BlockStateId)->bool{s.0==1}
 fn is_water(&self,s:BlockStateId)->bool{s.0==2}
 fn is_lava(&self,s:BlockStateId)->bool{s.0==3}
}
fn main()->Result<()> {
 let a:Vec<_>=env::args().collect();assert_eq!(a.len(),4,"METADATA_WORLDGEN PLAN.csv OUTPUT");
 let data=Path::new(&a[1]);let out=Path::new(&a[3]);fs::create_dir_all(out)?;
 let noises=bundle(&data.join("noise"))?;let defs=bundle(&data.join("density_function"))?;
 let settings:serde_json::Value=serde_json::from_str(&fs::read_to_string(data.join("noise_settings/overworld.json"))?)?;
 let catalog=CarverCatalog::from_json(&bundle(&data.join("carver"))?)?;
 let selected=catalog.decode_set(r#"["minecraft:cave","minecraft:cave_extra_underground","minecraft:canyon"]"#)?;
 let mut previous=None;let mut compiler:Option<DensityCompiler>=None;let mut final_density:Option<Arc<dyn DensitySampler>>=None;let mut aquifer_roots:Option<[Arc<dyn DensitySampler>;6]>=None;
 for line in fs::read_to_string(&a[2])?.lines().skip(1) {
  let r:Vec<_>=line.split(',').collect();let seed:u64=r[1].parse()?;let x:i32=r[2].parse()?;let z:i32=r[3].parse()?;let n:usize=r[4].parse()?;
  assert!(n%16==0&&n<=256&&x%16==0&&z%16==0);
  if previous!=Some(seed) {
   let mut registry=DensityRegistry::from_json(&noises,&defs)?;
   let density_id=registry.reference("minecraft:overworld/final_density").ok_or("missing final density")?;
   let ac=AquiferConfig::from_json(&settings["aquifers"].to_string(),&mut registry)?;
   let mut c=DensityCompiler::new_seeded(registry.into_graph(),seed as i64,false);
   final_density=Some(c.get_sampler(density_id)?);
   let aq=ac.compile(&mut c)?;
   aquifer_roots=Some(std::array::from_fn(|i|aq.get(AquiferField::ALL[i]).clone()));
   compiler=Some(c);previous=Some(seed);
  }
  let _keep_compiler_alive=&compiler;
  let field=final_density.as_ref().unwrap();let root=XoroshiroRandom::new(seed as i64).fork_positional();
  let mut cells=vec![0u8;n*n*384];let mut surface=vec![-64i16;n*n];
  let mut context=SamplerContext::cached();
  for cz in 0..n/16 {for cx in 0..n/16 {
   let ox=x+cx as i32*16;let oz=z+cz as i32*16;
   let volume=DensityVolume::new([16,384,16],[ox,-64,oz],[1;3])?;
   let mut density=vec![0f32;volume.size()];context.reset();field.sample_volume(&mut context,&mut density,volume)?;
   let mut aq=NoiseBasedAquifer::new(AquiferSamplers::new(aquifer_roots.as_ref().unwrap().clone()),GlobalFluidPicker::new(63,BlockStateId(2),BlockStateId(3),BlockStateId(1)),root.from_hash_of("minecraft:aquifer").fork_positional(),States,volume,&mut context)?;
   for zz in 0..16 {for xx in 0..16 {for yy in 0..384 {
    let d=density[(zz*16+xx)*384+yy];let at=[ox+xx as i32,yy as i32-64,oz+zz as i32];
    let state=if d>0. {0} else {aq.compute_substance(&mut context,at,f64::from(d))?.map_or(0,|s|s.0 as u8)};
    cells[((cz*16+zz)*n+cx*16+xx)*384+yy]=state;
   }}}
   let mask=generate_carving_mask(seed as i64,ChunkPos::new(ox/16,oz/16),WorldGenerationContext::new(-64,384,63),false,|_|Ok::<_,std::convert::Infallible>(selected.as_slice()))?;
   for col in mask.columns() {for yy in (col.min_y..=col.max_y).rev() {
    let i=((cz*16+col.z as usize)*n+cx*16+col.x as usize)*384+(yy+64) as usize;
    {if let Some(state)=aq.compute_substance(&mut context,[ox+col.x,yy,oz+col.z],0.)? {cells[i]=state.0 as u8;}}
   }}
  }}
  for zz in 0..n {for xx in 0..n {let i=zz*n+xx;for yy in 0..384 {if cells[i*384+yy]==0 {surface[i]=yy as i16-64;}}}}
  let path=out.join(format!("{}.rcv",r[0]));let mut f=fs::OpenOptions::new().create_new(true).write(true).open(&path)?;
  f.write_all(b"RCV1")?;for v in [n as i16,384,-64] {f.write_all(&v.to_le_bytes())?;}f.write_all(&seed.to_le_bytes())?;for v in [x,z]{f.write_all(&v.to_le_bytes())?;}for v in surface{f.write_all(&v.to_le_bytes())?;}f.write_all(&cells)?;
  println!("{}",path.display());
 }
 Ok(())
}
