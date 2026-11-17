// SPDX-License-Identifier: MIT OR Apache-2.0
//! Analysis adapter executing pinned Horizon roots, without importing game assets.
//! Surface search is a coarse terrain proxy. --density scans the actual final
//! graph at every integer Y; it excludes carvers/aquifers/materials/decorations.
use horizon_core::density::{SamplerContext, graph::DensityCompiler};
use horizon_core::noise::DensityVolume;
use horizon_world::worldgen::density::DensityRegistry;
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
    let args: Vec<_> = env::args().collect();
    let data = Path::new(
        args.get(1)
            .ok_or("WORLDGEN_DATA [--step=N] [--validation] [--density]")?,
    );
    let step: usize = args
        .iter()
        .find_map(|v| v.strip_prefix("--step="))
        .unwrap_or("32")
        .parse()?;
    assert!(matches!(step, 16 | 32 | 64 | 128));
    let density = args.iter().any(|v| v == "--density");
    let seeds: &[u64] = if args.iter().any(|v| v == "--validation") {
        &[123, 777, 4099, 900001]
    } else {
        &[0, 42, 2026, u64::MAX]
    };
    let noises = bundle(&data.join("noise"))?;
    let mut defs = bundle(&data.join("density_function"))?;
    if args.iter().any(|v| v == "--uncaved") {
        // The same declared terrain-only analysis control as cave_reference.
        // This is executed Horizon geometry with cave graph branches ablated,
        // not an unmodified vanilla-world export or a RustCraft approximation.
        fn terrain_only(v: serde_json::Value) -> serde_json::Value {
            match v {
                serde_json::Value::String(ref s) if s.starts_with("minecraft:overworld/caves/") => {
                    serde_json::json!(32.)
                }
                serde_json::Value::Object(ref o)
                    if o.get("type").and_then(|v| v.as_str()) == Some("minecraft:range_choice")
                        && o.get("input").and_then(|v| v.as_str())
                            == Some("minecraft:overworld/sloped_cheese") =>
                {
                    serde_json::json!("minecraft:overworld/sloped_cheese")
                }
                serde_json::Value::Object(o) => serde_json::Value::Object(
                    o.into_iter().map(|(k, v)| (k, terrain_only(v))).collect(),
                ),
                serde_json::Value::Array(a) => {
                    serde_json::Value::Array(a.into_iter().map(terrain_only).collect())
                }
                other => other,
            }
        }
        let key = "minecraft:overworld/final_density";
        let control = terrain_only(serde_json::from_str(
            defs.get(key).ok_or("missing final density")?,
        )?);
        defs.insert(key.into(), control.to_string());
    }
    if args.iter().any(|v| v == "--router") {
        let names = [
            "continents",
            "erosion",
            "ridges",
            "ridges_folded",
            "offset",
            "factor",
            "jaggedness",
            "depth",
            "sloped_cheese",
            "base_3d_noise",
            "final_density",
        ];
        println!("seed,x,y,z,{}", names.join(","));
        for &seed in seeds {
            let registry = DensityRegistry::from_json(&noises, &defs)?;
            let ids: Vec<_> = names
                .iter()
                .map(|n| {
                    registry
                        .reference(&format!("minecraft:overworld/{n}"))
                        .unwrap()
                })
                .collect();
            let mut compiler =
                DensityCompiler::new_seeded(registry.into_graph(), seed as i64, false);
            let fields: Vec<_> = ids
                .into_iter()
                .map(|id| compiler.get_sampler(id).unwrap())
                .collect();
            let mut context = SamplerContext::cached();
            for [x, z] in [
                [0, 10],
                [-1, -1],
                [15, 16],
                [-17, -16],
                [-4096, 4095],
                [-24000, 20000],
                [28000, -30000],
                [-32768, 32767],
                [32767, -32768],
            ] {
                for y in [-64, 0, 63, 128, 255, 319] {
                    context.reset();
                    let values: Vec<_> = fields
                        .iter()
                        .map(|s| {
                            s.sample_value(&mut context, [x, y, z])
                                .map(|v| format!("{v:.9}"))
                        })
                        .collect::<std::result::Result<_, _>>()?;
                    println!("{seed},{x},{y},{z},{}", values.join(","));
                }
            }
        }
        return Ok(());
    }
    println!("seed,region,x,z,height,slope,c,e,w,offset,amplitude,biome");
    for &seed in seeds {
        let registry = DensityRegistry::from_json(&noises, &defs)?;
        let id = registry
            .reference(if density {
                "minecraft:overworld/final_density"
            } else {
                "minecraft:overworld/chunk_surface_level"
            })
            .ok_or("missing terrain root")?;
        let mut compiler = DensityCompiler::new_seeded(registry.into_graph(), seed as i64, false);
        let field = compiler.get_sampler(id)?;
        let mut context = SamplerContext::cached();
        for (region, (ox, oz, size)) in [
            (-4096, -4096, 8192),
            (-24000, 20000, 2048),
            (28000, -30000, 2048),
        ]
        .into_iter()
        .enumerate()
        {
            for z in (oz..oz + size).step_by(step) {
                for x in (ox..ox + size).step_by(step) {
                    context.reset();
                    let h = if density {
                        if args.iter().any(|v| v == "--scalar") {
                            let mut top = -64;
                            for y in (-64..320).rev() {
                                if field.sample_value(&mut context, [x, y, z])? > 0. {
                                    top = y;
                                    break;
                                }
                            }
                            top
                        } else {
                            let mut samples = [0f32; 384];
                            field.sample_volume(
                                &mut context,
                                &mut samples,
                                DensityVolume::new([1, 384, 1], [x, -64, z], [1; 3])?,
                            )?;
                            samples
                                .iter()
                                .rposition(|v| *v > 0.)
                                .map_or(-64, |i| i as i32 - 64)
                        }
                    } else {
                        field.sample_value(&mut context, [x, 0, z])? as i32
                    };
                    // Sea-relative heights in the same CSV schema. Slope is not
                    // inferred from the quantized surface proxy; analyzer uses
                    // explicit multi-scale differences of each measured grid.
                    println!("{seed},{region},{x},{z},{},0,0,0,0,0,0,255", h - 61);
                }
            }
        }
    }
    Ok(())
}
