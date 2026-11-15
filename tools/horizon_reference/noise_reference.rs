// SPDX-License-Identifier: MIT OR Apache-2.0
// Analysis adapter authored by RustCraft contributors. Uses unmodified Horizon.
use horizon_core::{
    noise::{Noise, NormalNoise},
    random::{PositionalRandom, RandomSource, XoroshiroRandom},
};
use std::{fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).unwrap();
    let names = [
        "cave_cheese",
        "cave_layer",
        "cave_entrance",
        "spaghetti_3d_1",
        "spaghetti_3d_2",
        "spaghetti_3d_rarity",
        "spaghetti_3d_thickness",
        "spaghetti_roughness",
        "spaghetti_roughness_modulator",
        "spaghetti_2d",
        "spaghetti_2d_elevation",
        "spaghetti_2d_modulator",
        "spaghetti_2d_thickness",
        "pillar",
        "pillar_rareness",
        "pillar_thickness",
        "noodle",
        "noodle_thickness",
        "noodle_ridge_a",
        "noodle_ridge_b",
    ];
    println!("seed,noise,x,y,z,value_bits");
    for seed in [0u64, 42, 2026, u64::MAX] {
        let root = XoroshiroRandom::new(seed as i64).fork_positional();
        for (index, name) in names.iter().enumerate() {
            let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(
                Path::new(&dir).join(format!("{name}.json")),
            )?)?;
            let mut builder = NormalNoise::builder();
            builder
                .set_base_octave(value["base_octave"].as_i64().unwrap() as i32)
                .set_base_amplitude(value["base_amplitude"].as_f64().unwrap());
            builder.set_octave_count(value["octave_count"].as_i64().unwrap_or(1) as i32)?;
            if let Some(v) = value["amplitude_modifiers"].as_array() {
                for (i, a) in v.iter().enumerate() {
                    builder.set_amplitude_modifier(i as i32, a.as_f64().unwrap())?;
                }
            }
            let stack = builder
                .build()?
                .create(&mut root.from_hash_of(&format!("minecraft:{name}")))?;
            for p in [
                [0, -64, 10],
                [-1, -1, -1],
                [15, -17, 16],
                [-129, 63, 127],
                [28000, 120, -30000],
                [-32768, -59, 32767],
                [32767, 190, -32768],
                [-253, -33, -129],
                [64, 0, 64],
            ] {
                println!(
                    "{seed},{index},{},{},{},{}",
                    p[0],
                    p[1],
                    p[2],
                    stack.get3(p[0] as f64, p[1] as f64, p[2] as f64).to_bits()
                );
            }
        }
    }
    Ok(())
}
