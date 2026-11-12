#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Exact merged baseline/current headless streaming comparison; no production probes.

Both executables build first, each core is forcibly recompiled, and executable
source fingerprints are asserted at runtime. Raw events and all outliers retained.
"""

import argparse, csv, hashlib, io, json, os, platform, shutil, subprocess, tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASE = "1b7e405"


def replace(path, old, new):
    s = path.read_text()
    assert s.count(old) == 1, (path, old, s.count(old))
    path.write_text(s.replace(old, new))


def fingerprint(root, dirs):
    return hashlib.sha256(
        b"".join(
            str(p.relative_to(root)).encode() + b"\0" + p.read_bytes()
            for d in dirs
            for p in sorted((root / d).rglob("*.rs"))
        )
    ).hexdigest()


def instrument(root, current):
    core = root / "crates/voxel_core/src/streaming.rs"
    stream = root / "client/src/streaming.rs"
    replace(
        core,
        "        let chunks = cache.columns.entry(c).or_insert_with(|| {",
        "        S32_CACHE.with(|v| { let (h,m)=v.get(); v.set(if cache.columns.contains_key(&c){(h+1,m)}else{(h,m+1)}); });\n        let chunks = cache.columns.entry(c).or_insert_with(|| {",
    )
    core.write_text(
        core.read_text()
        + """\nthread_local!{static S32_CACHE:std::cell::Cell<(usize,usize)>=const{std::cell::Cell::new((0,0))};}
pub fn slice32_cache_stats()->(usize,usize){S32_CACHE.with(|v|v.get())}
"""
    )
    core_hash = fingerprint(root, ["crates/voxel_core/src"])
    core.write_text(
        core.read_text()
        + f'\npub fn slice32_fingerprint()->&\'static str{{"{core_hash}"}}\n'
    )
    client_hash = fingerprint(root, ["client/src"])
    probe = (
        ROOT / "tools/streaming_bench/probe.rs"
    ).read_text() + f'\npub const SOURCE:&str="{client_hash}";\n'
    (root / "client/src/slice32_probe.rs").write_text(probe)
    main = root / "client/src/main.rs"
    main.write_text(main.read_text() + "\n#[cfg(test)] mod slice32_probe;\n")
    pre = "crate::slice32_probe"
    replace(
        stream,
        "            .spawn(move || {",
        "            .spawn(move || {\n                let _bench_worker=crate::slice32_probe::Worker::new();",
    )
    replace(
        stream,
        "edits.insert_region(p, store.read_region(id, p)?);",
        'edits.insert_region(p, {let start=Instant::now();let value=store.read_region(id,p);crate::slice32_probe::duration("region_read_us",start.elapsed());value?});',
    )
    replace(
        stream,
        "        if let Some(work) = work {",
        """        if let Some(work) = work {
            let bench_load=match &work{Work::Load(c)=>Some(*c),_=>None};
            crate::slice32_probe::dispatch(bench_load,&work.targets(world.0.world_height()),&state.queued_at,&state.dirty);
            let bench_prepare=Instant::now();""",
    )
    replace(
        stream,
        "                Ok(job) => {",
        '                Ok(job) => {\n                    crate::slice32_probe::duration("prepare_us",bench_prepare.elapsed());',
    )
    replace(
        stream,
        (
            "                            let install_start = Instant::now();"
            if current
            else "                    let install_start = Instant::now();"
        ),
        ("                            " if current else "                    ")
        + """let install_start = Instant::now();
                            let bench_load=match &job.work{Work::Load(c)=>Some(*c),_=>None};
                            let bench_install=crate::slice32_probe::Install::new(bench_load,&job.work.targets(world.0.world_height()),&state.queued_at,&state.dirty);""",
    )
    replace(
        stream,
        (
            "                            if matches!(job.work, Work::Mesh(_)) {"
            if current
            else "                    if matches!(job.work, Work::Mesh(_)) {"
        ),
        "                            bench_install.ready();\n"
        + ("                            " if current else "                    ")
        + "if matches!(job.work, Work::Mesh(_)) {",
    )
    if current:
        replace(
            stream,
            "    fn record_cancel(&mut self, stage: streaming::Cancelled) {",
            '    fn record_cancel(&mut self, stage: streaming::Cancelled) {\n        crate::slice32_probe::value("cancel_event",1.);',
        )
    # Baseline removes the timestamp early; the install guard captured it above.
    fixture = (ROOT / "tools/streaming_bench/workloads.rs").read_text()
    fixture = fixture.replace(
        "// S32_EXTRA_METRICS",
        (
            'probe::value("cancelled",state.metrics.cancelled as f64);probe::value("cancelled_before_mesh",state.metrics.cancelled_before_mesh as f64);probe::value("failed",state.metrics.failed as f64);'
            if current
            else 'probe::value("cancelled",0.);probe::value("cancelled_before_mesh",0.);probe::value("failed",0.);'
        ),
    )
    tests = root / "client/src/streaming/tests.rs"
    distance_fixture = (ROOT / "tools/streaming_bench/distance.rs").read_text()
    if not current:
        # Slice31 has no first-dispatch tracking sets. Keep its original default
        # reproduction buildable even when the expansion helpers are unused.
        distance_fixture = distance_fixture.replace(
            "    assert!(state.first_load_dispatch.is_empty() && state.first_mesh_dispatch.is_empty());\n", "")
    tests.write_text(tests.read_text() + "\n" + fixture + "\n" + distance_fixture)
    sessions = root / "client/src/session/tests.rs"
    sessions.write_text(
        sessions.read_text()
        + "\n"
        + (ROOT / "tools/streaming_bench/sessions.rs").read_text()
    )
    return core_hash, client_hash


def instrument_rendering(root):
    """Additional CPU-only boundaries, identical for both Slice32+ exports."""
    stream = root / "client/src/streaming.rs"
    replace(stream, "        match job.handle.join() {", '''        match {
            let start=Instant::now(); let result=job.handle.join();
            crate::slice32_probe::duration("worker_join_us",start.elapsed()); result
        } {''')
    scene = root / "client/src/scene.rs"
    text = scene.read_text()
    assert text.count("    let mesh = Mesh::new(") == 2
    text = text.replace("    let mesh = Mesh::new(", "    let construct_start=std::time::Instant::now();\n    let mesh = Mesh::new(")
    marker = ".with_inserted_indices(Indices::U32(surface.indices));"
    assert text.count(marker) == 2
    text = text.replace(marker, marker + '\n    crate::slice32_probe::duration("bevy_mesh_construct_us",construct_start.elapsed());')
    scene.write_text(text)


def instrument_distance(root):
    """Identical stage/visibility probes for the radius expansion; no game probes."""
    from benchmark_stabilization import instrument as stages
    stages(root)
    core = root / "crates/voxel_core/src/streaming.rs"
    replace(core, "            let sampler = generator.sampler(c);",
            "            let generation_start=Instant::now();\n            let sampler = generator.sampler(c);")
    replace(core, "                .collect()\n        });",
            "                .collect::<Vec<_>>();\n            crate::worldgen::slice30_add(15,generation_start.elapsed().as_nanos());\n            generated_chunks\n        });")
    # Name the generation closure result so the timer covers the complete column.
    replace(core, "            height\n                .chunks()", "            let generated_chunks = height\n                .chunks()")
    stream = root / "client/src/streaming.rs"
    replace(stream, "                timing.wall = worker_start.elapsed();", '''                timing.wall = worker_start.elapsed();
                crate::slice32_probe::value("worker_cache_columns", cache.columns() as f64);
                crate::slice32_probe::value("worker_cache_bytes", cache.allocated_voxel_bytes() as f64);''')
    replace(stream, "    state.max_update = state.max_update.max(update_start.elapsed());", '''    state.max_update = state.max_update.max(update_start.elapsed());
    crate::slice32_probe::duration("stream_update_us",update_start.elapsed());
    crate::slice32_probe::value("resident_descriptors_update",world.0.chunk_count() as f64);
    crate::slice32_probe::value("pending_loads_update",state.queued.len() as f64);
    crate::slice32_probe::value("pending_dirty_update",state.dirty.len() as f64);
    crate::slice32_probe::value("resident_mesh_bytes_update",state.mesh_bytes as f64);''')
    probe = root / "client/src/slice32_probe.rs"
    replace(probe, '        duration("worker_wall_us", wall);', '''        let stages=voxel_core::worldgen::slice30_take();
        for (metric,index) in [("generation_us",15),("lighting_us",2),("meshing_us",3),("water_mesh_us",4),("plant_mesh_us",5)] {
            value(metric,stages[index] as f64/1000.);
        }
        duration("worker_wall_us", wall);''')
    tests = root / "client/src/streaming/tests.rs"
    before, after = tests.read_text().split("fn large_app_with_generator(", 1)
    marker = "    app.add_plugins(MinimalPlugins)\n        .insert_resource("
    assert after.count(marker) == 1
    after = after.replace(marker, '''    app.add_plugins(MinimalPlugins);
    if std::env::var("S34_DISTANCE").as_deref()==Ok("1") {
        app.add_plugins((bevy::asset::AssetPlugin::default(),bevy::mesh::MeshPlugin,
            bevy::transform::TransformPlugin,bevy::camera::visibility::VisibilityPlugin));
        app.add_systems(PostUpdate,crate::camera::rebase_scene.before(bevy::transform::TransformSystems::Propagate));
    }
    app.insert_resource(''')
    tests.write_text(before + "fn large_app_with_generator(" + after)
    camera = root / "client/src/camera.rs"
    if "\nfn rebase_scene(" in camera.read_text():
        replace(camera, "\nfn rebase_scene(", "\npub(crate) fn rebase_scene(")


def summarize_traces(paths, out):
    """Preserve all valid trace observations; report malformed lines explicitly."""
    groups = {}
    malformed = []
    for path in paths:
        for number, line in enumerate(path.read_text(errors="replace").splitlines(), 1):
            if "S32_TRACE," not in line:
                continue
            try:
                _, metric, value = line[line.index("S32_TRACE,") :].split(",")
                value = float(value)
            except ValueError:
                malformed.append(f"{path.name}:{number}: {line}")
                continue
            groups.setdefault((path.name, metric), []).append(value)
    if not groups:
        raise ValueError("no S32_TRACE observations found")
    with (out / "trace-percentiles.csv").open("w", newline="") as file:
        writer = csv.writer(file, lineterminator="\n")
        writer.writerow(
            ["file", "metric", "count", "p50", "p95", "p99", "total", "max"]
        )
        for key, values in sorted(groups.items()):
            values.sort()
            writer.writerow(
                [
                    *key,
                    len(values),
                    *(values[(len(values) - 1) * p // 100] for p in [50, 95, 99]),
                    sum(values),
                    max(values),
                ]
            )
    (out / "trace-parse-errors.txt").write_text(
        "\n".join(malformed) + ("\n" if malformed else "")
    )
    print(f"{out / 'trace-percentiles.csv'}; malformed trace lines: {len(malformed)}")


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--baseline", default=BASE)
    p.add_argument("--labels", nargs=2, default=["slice31", "slice32"],
                   help="Distinct baseline/current output labels for later slices.")
    p.add_argument("--reverse", action="store_true")
    p.add_argument("--repeats", type=int, default=3)
    p.add_argument("--frames", type=int, default=120)
    p.add_argument("--prepare-only", action="store_true")
    p.add_argument(
        "--build-only",
        action="store_true",
        help="Build both verified executables without measuring yet.",
    )
    p.add_argument(
        "--native",
        action="store_true",
        help="Build optional instrumented GUI executables instead of running headless tests.",
    )
    p.add_argument("--reuse", action="store_true")
    p.add_argument("--summarize-only", action="store_true", help="Aggregate existing verified raw runs without running or rebuilding them")
    p.add_argument("--terrain-v4", action="store_true", help="Matched exact Slice35 v10/current v11; enables terrain scenes")
    p.add_argument("--terrain-v3", action="store_true", help="Matched Slice34 v9/current v10 terrain scenes; use with --distance-expansion")
    p.add_argument("--rendering-install", action="store_true",
                   help="Also measure retained connected batches with real Bevy CPU visibility.")
    p.add_argument("--distance-expansion", action="store_true",
                   help="Measure supported radii through 4, dense edits, stage costs and real CPU visibility.")
    p.add_argument(
        "--trace",
        nargs="+",
        type=Path,
        help="Summarize native S32_TRACE logs without building or running tests.",
    )
    a = p.parse_args()
    if a.terrain_v4: a.terrain_v3 = True
    if a.summarize_only: a.reuse = True
    assert 1 <= a.repeats <= 100 and 1 <= a.frames <= 10000
    out = a.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    if a.trace:
        summarize_traces(a.trace, out)
        return
    scratch = ROOT / "work/streaming-benchmark"
    baseline = subprocess.check_output(
        ["git", "rev-parse", a.baseline], cwd=ROOT, text=True
    ).strip()
    names = a.labels
    assert names[0].casefold() != names[1].casefold() and all(n.replace("-", "").replace("_", "").isalnum() for n in names)
    sources = {}
    ids = {}
    if not a.reuse:
        for name in names:
            root = scratch / name
            if root.exists():
                shutil.rmtree(root)
            root.mkdir(parents=True)
            archive = subprocess.check_output(
                [
                    "git",
                    "archive",
                    baseline,
                    "Cargo.toml",
                    "Cargo.lock",
                    "rust-toolchain.toml",
                    "client",
                    "crates/voxel_core",
                    "assets",
                ],
                cwd=ROOT,
            )
            with tarfile.open(fileobj=io.BytesIO(archive)) as t:
                t.extractall(root, filter="data")
            if name == names[1]:
                for d in ["client", "crates/voxel_core"]:
                    shutil.rmtree(root / d)
                    shutil.copytree(ROOT / d, root / d)
            sources[name] = {
                str(f.relative_to(root)): hashlib.sha256(f.read_bytes()).hexdigest()
                for d in ["client/src", "crates/voxel_core/src"]
                for f in sorted((root / d).rglob("*.rs"))
            }
            ids[name] = instrument(root, "fn record_cancel(" in (root / "client/src/streaming.rs").read_text())
            if a.terrain_v3:
                tests = root / "client/src/streaming/tests.rs"
                s = tests.read_text()
                s = s.replace('generator:9,', 'generator:std::env::var("S35_GENERATOR").unwrap().parse().unwrap(),')
                s = s.replace('large_app_with_generator(path,9)', 'large_app_with_generator(path,std::env::var("S35_GENERATOR").unwrap().parse().unwrap())')
                s = s.replace('"origin"]);', '"origin", "mountain", "valley"]);')
                s = s.replace('"coast"|"cavern"|"origin"', '"coast"|"cavern"|"origin"|"valley"')
                s = s.replace('"forest" => [-136, -256],', '"mountain" => [-249,-197], "valley" => [-180,45], "forest" => [-136,-256],')
                tests.write_text(s)
            if a.rendering_install:
                tests = root / "client/src/scene/tests.rs"
                tests.write_text(tests.read_text() + "\n" + (ROOT / "tools/rendering_bench/install.rs").read_text())
                instrument_rendering(root)
            if a.distance_expansion:
                instrument_distance(root)
        (out / "sources.json").write_text(
            json.dumps(
                {"baseline": baseline, "files": sources, "runtime_ids": ids,
                 "probes": {"distance_expansion": a.distance_expansion, "rendering_install": a.rendering_install},
                 "terrain_v3": a.terrain_v3,
                 "terrain_v4": a.terrain_v4,
                 "harness_sha256": {str(f.relative_to(ROOT)): hashlib.sha256(f.read_bytes()).hexdigest()
                    for f in [Path(__file__).resolve(), ROOT / "tools/rendering_bench/install.rs", *sorted((ROOT / "tools/streaming_bench").glob("*.rs"))]}}, indent=2
            )
            + "\n"
        )
        if a.prepare_only:
            return
        if a.native:
            for name in names:
                root = scratch / name
                main = root / "client/src/main.rs"
                replace(main, "#[cfg(test)] mod slice32_probe;", "mod slice32_probe;")
                replace(
                    main,
                    "fn main() {",
                    f"""fn main() {{
    assert_eq!(voxel_core::streaming::slice32_fingerprint(), "{ids[name][0]}");
    eprintln!("S32_SOURCE,{name},{ids[name][0]},{ids[name][1]}");""",
                )
                for d in ["client", "crates/voxel_core"]:
                    for f in (root / d).rglob("*.rs"):
                        f.touch()
                with (out / f"{name}-native-build.log").open("w") as log:
                    subprocess.run(
                        [
                            "cargo",
                            "clean",
                            "--release",
                            "-p",
                            "voxel_core",
                            "--target-dir",
                            str(ROOT / "target"),
                        ],
                        cwd=root,
                        stdout=log,
                        stderr=log,
                        check=True,
                    )
                    subprocess.run(
                        [
                            "cargo",
                            "build",
                            "--release",
                            "--locked",
                            "--target-dir",
                            str(ROOT / "target"),
                            "-p",
                            "rustcraft",
                        ],
                        cwd=root,
                        stdout=log,
                        stderr=log,
                        check=True,
                    )
                suffix = ".exe" if os.name == "nt" else ""
                shutil.copy2(
                    ROOT / f"target/release/rustcraft{suffix}",
                    out / f"{name}-native{suffix}",
                )
            return
        for name in names:
            root = scratch / name
            for d in ["client", "crates/voxel_core"]:
                for f in (root / d).rglob("*.rs"):
                    f.touch()
            with (out / f"{name}-build.log").open("w") as log:
                subprocess.run(
                    [
                        "cargo",
                        "clean",
                        "--release",
                        "-p",
                        "voxel_core",
                        "--target-dir",
                        str(ROOT / "target"),
                    ],
                    cwd=root,
                    stdout=log,
                    stderr=log,
                    check=True,
                )
                built = subprocess.run(
                    [
                        "cargo",
                        "test",
                        "--release",
                        "--locked",
                        "--target-dir",
                        str(ROOT / "target"),
                        "-p",
                        "rustcraft",
                        "--bin",
                        "rustcraft",
                        "--no-run",
                        "--message-format=json",
                    ],
                    cwd=root,
                    stdout=subprocess.PIPE,
                    stderr=log,
                    text=True,
                    check=True,
                )
            assert "Compiling voxel_core " in (out / f"{name}-build.log").read_text()
            binaries = [
                j["executable"]
                for l in built.stdout.splitlines()
                if (j := json.loads(l)).get("reason") == "compiler-artifact"
                and j.get("executable")
                and j.get("profile", {}).get("test")
            ]
            assert len(binaries) == 1
            shutil.copy2(binaries[0], out / f"{name}-bench")
        (out / "executables-sha256.json").write_text(json.dumps({
            name: hashlib.sha256((out / f"{name}-bench").read_bytes()).hexdigest()
            for name in names}, indent=2) + "\n")
    if a.build_only:
        return
    manifest = json.loads((out / "sources.json").read_text())
    assert manifest["baseline"] == baseline, "Reused baseline differs from requested revision"
    if "probes" in manifest:
        assert manifest["probes"] == {"distance_expansion": a.distance_expansion, "rendering_install": a.rendering_install}, "Reused probes differ"
    assert manifest.get("terrain_v3", False) == a.terrain_v3, "Reused terrain mode differs"
    assert manifest.get("terrain_v4", False) == a.terrain_v4, "Reused terrain v4 mode differs"
    ids = manifest["runtime_ids"]
    assert set(ids) == set(names), "Reused build labels differ"
    executables = json.loads((out / "executables-sha256.json").read_text())
    for name in names:
        assert hashlib.sha256((out / f"{name}-bench").read_bytes()).hexdigest() == executables[name], "Reused executable changed"
    for name in ([] if a.summarize_only else (list(reversed(names)) if a.reverse else names)):
        with (out / f"{name}.txt").open("w") as log:
            subprocess.run(
                [
                    str(out / f"{name}-bench"),
                    "slice32_streaming_measurements",
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ],
                cwd=scratch / name,
                stdout=log,
                stderr=log,
                check=True,
                env={
                    **os.environ,
                    "S32_REPEATS": str(a.repeats),
                    "S32_FRAMES": str(a.frames),
                    "S32_EXPECT_CORE": ids[name][0],
                    "S32_EXPECT_CLIENT": ids[name][1],
                    "S34_DISTANCE": str(int(a.distance_expansion)),
                    "S35_GENERATOR": ("11" if name == names[1] else "10") if a.terrain_v4 else ("10" if a.terrain_v3 and name == names[1] else "9"),
                },
            )
            subprocess.run(
                [
                    str(out / f"{name}-bench"),
                    "slice32_session_measurements",
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ],
                cwd=scratch / name,
                stdout=log,
                stderr=log,
                check=True,
                env={
                    **os.environ,
                    "S32_REPEATS": str(a.repeats),
                    "S32_EXPECT_CORE": ids[name][0],
                    "S32_EXPECT_CLIENT": ids[name][1],
                    "S34_DISTANCE": str(int(a.distance_expansion)),
                    "S35_GENERATOR": ("11" if name == names[1] else "10") if a.terrain_v4 else ("10" if a.terrain_v3 and name == names[1] else "9"),
                },
            )
            if a.rendering_install:
                subprocess.run(
                    [str(out / f"{name}-bench"), "slice33_install_measurements", "--ignored", "--nocapture", "--test-threads=1"],
                    cwd=scratch / name, stdout=log, stderr=log, check=True,
                    env={**os.environ, "S32_REPEATS": str(a.repeats), "S32_EXPECT_CORE": ids[name][0], "S32_EXPECT_CLIENT": ids[name][1], "S34_DISTANCE": str(int(a.distance_expansion))},
                )
        print(out / f"{name}.txt", flush=True)
    states = {}
    samples = []
    for name in names:
        states[name] = [
            l[l.index("S32_STATE,") :]
            for l in (out / f"{name}.txt").read_text().splitlines()
            if "S32_STATE," in l
        ]
        for l in (out / f"{name}.txt").read_text().splitlines():
            if "S32_SAMPLE," in l:
                _, scene, radius, repeat, metric, index, value = l[
                    l.index("S32_SAMPLE,") :
                ].split(",")
                samples.append([name, scene, radius, repeat, metric, index, value])
    # The Rust harness may prefix the first state with its test-name banner.
    if a.terrain_v3:
        # Slice34 already supports radius4; terrain versions intentionally have
        # different block/resource signatures. Compare identical workload keys.
        keyed = {name: {tuple(s.split(",")[1:4]): s for s in states[name]} for name in names}
        expected={(scene,str(radius),str(repeat)) for scene in [
            "stationary","walking","sprinting","diagonal","reversal","teleport",
            "gated_teleport","return","forest","coast","cavern","editing",
            "dirty_pressure","water","radius","dense-construction","edited-water",
            "origin","mountain","valley","session"]
            for radius in range(1,5) for repeat in range(a.repeats)}
        assert all(set(keyed[name])==expected for name in names), "Incomplete terrain workload coverage"
    elif a.distance_expansion:
        # The exact baseline cannot run radius4. Compare all common keys exactly,
        # retain expansion-only states separately, and check coverage explicitly.
        keyed = {name: {tuple(s.split(",")[1:4]): s for s in states[name]} for name in names}
        common = keyed[names[0]].keys() & keyed[names[1]].keys()
        assert len(common) == 19 * 3 * a.repeats
        assert all(keyed[names[0]][k] == keyed[names[1]][k] for k in common), "Final authority/resources differ"
        assert len(states[names[1]]) == 19 * 4 * a.repeats
    else:
        assert len(states[names[0]]) == len(states[names[1]]) == 16 * 3 * a.repeats
        assert states[names[0]] == states[names[1]], "Final authority/resources differ: inspect raw states"
    (out / "states.json").write_text(json.dumps(states, indent=2) + "\n")
    with (out / "samples.csv").open("w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["build", "scene", "radius", "repeat", "metric", "index", "value"])
        w.writerows(samples)
    grouped = {}
    for name, scene, radius, repeat, metric, index, value in samples:
        grouped.setdefault((name, scene, radius, metric), []).append(float(value))
    with (out / "percentiles.csv").open("w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(
            [
                "build",
                "scene",
                "radius",
                "metric",
                "count",
                "p50",
                "p95",
                "p99",
                "total",
                "max",
            ]
        )
        for key, v in sorted(grouped.items()):
            v.sort()
            w.writerow(
                [
                    *key,
                    len(v),
                    *(f"{v[(len(v)-1)*p//100]:.6f}" for p in [50, 95, 99]),
                    f"{sum(v):.6f}",
                    f"{max(v):.6f}",
                ]
            )
    if a.distance_expansion:
        with (out / "comparison.csv").open("w", newline="") as f:
            w = csv.writer(f, lineterminator="\n")
            w.writerow(["comparison", "scene", "radius", "reference_radius", "metric", "base_n", "new_n",
                        *[f"{side}_{stat}" for stat in ["p50", "p95", "p99", "max"] for side in ["base", "new"]]])
            for (name, scene, radius, metric), values in sorted(grouped.items()):
                if name != names[1]:
                    continue
                reference_radius = "3" if radius == "4" and not a.terrain_v3 else radius
                reference_name = names[1] if radius == "4" and not a.terrain_v3 else names[0]
                base_values = grouped.get((reference_name, scene, reference_radius, metric))
                if base_values:
                    stats = lambda v: [v[(len(v)-1)*p//100] for p in [50,95,99]] + [max(v)]
                    comparison = "transition" if scene == "radius" else ("expansion" if radius == "4" and not a.terrain_v3 else "matched")
                    w.writerow([comparison, scene, radius, reference_radius, metric,
                                len(base_values), len(values), *[n for pair in zip(stats(base_values),stats(values)) for n in pair]])
    (out / "method.json").write_text(
        json.dumps(
            {
                "baseline": baseline,
                "order": list(reversed(names)) if a.reverse else names,
                "repeats": a.repeats,
                "frames": a.frames,
                "frame_hz": 240,
                "rendering_install": a.rendering_install,
                "distance_expansion": a.distance_expansion,
                "transition_scope": ("Terrain mode compares v10/v11 at matched radii1-4; geometry/resource signatures may differ. Radius stress visits4 in both builds." if a.terrain_v4 else "Terrain mode compares v9/v10 at matched radii1-4; geometry/resource signatures may differ. Radius stress visits4 in both builds." if a.terrain_v3 else "In distance mode, radius stress visits each build's own maximum (baseline3, candidate4); common final signatures match but these rows are not matched-workload regression evidence."),
                "aggregation_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                "machine": platform.platform(),
                "note": "Headless scripted position/velocity, fixed 240Hz deadlines, no rendered frames/GPU. RSS is process-wide; requested heap peak is additional process heap during worker window, excludes stacks/allocator metadata/GPU and may include concurrent main allocations.",
            },
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
