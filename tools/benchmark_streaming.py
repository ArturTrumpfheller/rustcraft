#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Exact merged Slice31/current headless streaming comparison; no production probes.

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
    tests.write_text(tests.read_text() + "\n" + fixture)
    sessions = root / "client/src/session/tests.rs"
    sessions.write_text(
        sessions.read_text()
        + "\n"
        + (ROOT / "tools/streaming_bench/sessions.rs").read_text()
    )
    return core_hash, client_hash


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--baseline", default=BASE)
    p.add_argument("--reverse", action="store_true")
    p.add_argument("--repeats", type=int, default=3)
    p.add_argument("--frames", type=int, default=120)
    p.add_argument("--prepare-only", action="store_true")
    p.add_argument(
        "--native",
        action="store_true",
        help="Build optional instrumented GUI executables instead of running headless tests.",
    )
    p.add_argument("--reuse", action="store_true")
    a = p.parse_args()
    assert 1 <= a.repeats <= 100 and 1 <= a.frames <= 10000
    out = a.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    scratch = ROOT / "work/streaming-benchmark"
    baseline = subprocess.check_output(
        ["git", "rev-parse", a.baseline], cwd=ROOT, text=True
    ).strip()
    names = ["slice31", "slice32"]
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
            if name == "slice32":
                for d in ["client", "crates/voxel_core"]:
                    shutil.rmtree(root / d)
                    shutil.copytree(ROOT / d, root / d)
            sources[name] = {
                str(f.relative_to(root)): hashlib.sha256(f.read_bytes()).hexdigest()
                for d in ["client/src", "crates/voxel_core/src"]
                for f in sorted((root / d).rglob("*.rs"))
            }
            ids[name] = instrument(root, name == "slice32")
        (out / "sources.json").write_text(
            json.dumps(
                {"baseline": baseline, "files": sources, "runtime_ids": ids}, indent=2
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
    ids = json.loads((out / "sources.json").read_text())["runtime_ids"]
    for name in reversed(names) if a.reverse else names:
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
                },
            )
        print(out / f"{name}.txt", flush=True)
    states = {}
    samples = []
    for name in names:
        states[name] = [
            l
            for l in (out / f"{name}.txt").read_text().splitlines()
            if l.startswith("S32_STATE,")
        ]
        for l in (out / f"{name}.txt").read_text().splitlines():
            if l.startswith("S32_SAMPLE,"):
                _, scene, radius, repeat, metric, index, value = l.split(",")
                samples.append([name, scene, radius, repeat, metric, index, value])
    assert (
        states[names[0]] == states[names[1]]
    ), "Final authority/resources differ: inspect raw states"
    with (out / "samples.csv").open("w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["build", "scene", "radius", "repeat", "metric", "index", "value"])
        w.writerows(samples)
    grouped = {}
    for name, scene, radius, repeat, metric, index, value in samples:
        grouped.setdefault((name, scene, radius, metric), []).append(float(value))
    with (out / "percentiles.csv").open("w", newline="") as f:
        w = csv.writer(f)
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
    (out / "method.json").write_text(
        json.dumps(
            {
                "baseline": baseline,
                "order": list(reversed(names)) if a.reverse else names,
                "repeats": a.repeats,
                "frames": a.frames,
                "frame_hz": 240,
                "machine": platform.platform(),
                "note": "Headless scripted position/velocity, fixed 240Hz deadlines, no rendered frames/GPU. RSS is process-wide; requested heap peak is additional process heap during worker window, excludes stacks/allocator metadata/GPU and may include concurrent main allocations.",
            },
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
