// SPDX-License-Identifier: MIT OR Apache-2.0
// RustCraft's analysis-only adapter. Horizon itself is unmodified MIT source.
// Reads external functional registry definitions; no game assets or JARs.
use horizon_core::density::{DensitySampler, SamplerContext, graph::DensityCompiler};
use horizon_world::{
    registry::BlockStateId,
    worldgen::{anchor::WorldGenerationContext, density::DensityRegistry},
};
use std::{collections::BTreeMap, env, fs, path::Path};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn bundle(dir: &Path) -> Result<BTreeMap<String, String>> {
    fn visit(dir: &Path, root: &Path, out: &mut BTreeMap<String, String>) -> Result<()> {
        for f in fs::read_dir(dir)? {
            let p = f?.path();
            if p.is_dir() {
                visit(&p, root, out)?;
            } else if p.extension().is_some_and(|v| v == "json") {
                out.insert(
                    format!(
                        "minecraft:{}",
                        p.strip_prefix(root)?
                            .with_extension("")
                            .to_string_lossy()
                            .replace('\\', "/")
                    ),
                    fs::read_to_string(p)?,
                );
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    visit(dir, dir, &mut out)?;
    Ok(out)
}
fn main() -> Result<()> {
    let data = env::args().nth(1).unwrap();
    let data = Path::new(&data);
    let noises = bundle(&data.join("noise"))?;
    let defs = bundle(&data.join("density_function"))?;
    println!("id,seed,x,z,n,stratum,attempt,mean_height,relief");
    for (si, seed) in [0u64, 42, 2026, u64::MAX].into_iter().enumerate() {
        let mut registry = DensityRegistry::from_json(&noises, &defs)?;
        let id = registry
            .reference("minecraft:overworld/chunk_surface_level")
            .unwrap();
        let mut compiler = DensityCompiler::new_seeded(registry.into_graph(), seed as i64, false);
        let field = compiler.get_sampler(id)?;
        for site in 4u64..8 {
            let mut selected = None;
            for attempt in 0u64..4096 {
                let mut h = seed ^ (site * 314159 + attempt * 271828 + 1);
                h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
                h ^= h >> 31;
                let x = ((h % 3584) as i32 - 1792) * 16;
                let z = (((h >> 24) % 3584) as i32 - 1792) * 16;
                let mut heights = vec![];
                let mut context = SamplerContext::cached();
                for dz in [0, 64, 127] {
                    for dx in [0, 64, 127] {
                        heights.push(field.sample_value(&mut context, [x + dx, 0, z + dz])? as i32);
                    }
                }
                let mean = heights.iter().sum::<i32>() / 9;
                let relief = heights.iter().max().unwrap() - heights.iter().min().unwrap();
                if (site < 6 && (82..=104).contains(&mean) && relief <= 28)
                    || (site >= 6 && mean >= 139 && relief >= 12)
                {
                    selected = Some((x, z, attempt, mean, relief));
                    break;
                }
            }
            let (x, z, attempt, mean, relief) =
                selected.ok_or("terrain stratum search exhausted")?;
            let stratum = if site < 6 {
                "lowland-conditional"
            } else {
                "mountain-conditional"
            };
            println!("s{si:02}-{site},{seed},{x},{z},128,{stratum},{attempt},{mean},{relief}");
        }
    }
    Ok(())
}
