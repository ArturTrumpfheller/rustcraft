// SPDX-License-Identifier: MIT OR Apache-2.0
//! Read-only, bounded multi-seed integration audit; never opens a save directory.
#[path = "support/worldgen_audit.rs"]
mod audit;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        args.is_empty() || args.len() == 1 || args.len() == 4,
        "usage: worldgen_audit [generator [seed chunk_x chunk_z]]"
    );
    let version = args.first().map_or(9, |v| v.parse().unwrap());
    assert!((3..=9).contains(&version));
    let sites: Vec<_> = if args.len() == 4 {
        vec![(
            args[1].parse().unwrap(),
            "custom",
            [args[2].parse().unwrap(), args[3].parse().unwrap()],
        )]
    } else {
        audit::SEEDS
            .into_iter()
            .flat_map(|seed| audit::SITES.map(|(name, c)| (seed, name, c)))
            .collect()
    };
    println!(
        "seed,generator,site,chunk_x,chunk_z,biome,crc,plants,wood,leaves,water,cave_air,underground_water,voxel_bytes,findings"
    );
    let mut failures = 0;
    for (seed, name, c) in sites {
        let r = audit::audit(seed, version, c);
        audit::parallel_parity(seed, version, c);
        println!(
            "{seed},{version},{name},{},{},{},{},{},{},{},{},{},{},{},{}",
            c[0],
            c[1],
            r.biome,
            r.crc,
            r.plants,
            r.wood,
            r.leaves,
            r.water,
            r.cave_air,
            r.underground_water,
            r.voxel_bytes,
            r.findings.len()
        );
        failures += r.findings.len();
        for finding in r.findings {
            eprintln!("{finding}");
        }
    }
    assert_eq!(
        failures, 0,
        "worldgen integration audit reported actionable findings"
    );
}
