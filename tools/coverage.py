"""Measure each Rust component; all product sources and child binaries are included."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import re

ROOT = Path(__file__).resolve().parents[1]


def run(args, **kwargs):
    return subprocess.run(args, cwd=ROOT, check=True, text=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--require-branches", action="store_true")
    parser.add_argument("--toolchain", default="nightly-2026-11-04")
    parser.add_argument("--output-dir", default="docs/specker/runs/2026-11-04-karate/evidence/keys")
    args = parser.parse_args()
    metadata = json.loads(run(["./scripts/cargo.sh", "metadata", "--no-deps", "--locked", "--offline", "--format-version=1"], capture_output=True).stdout)
    packages = [p for p in metadata["packages"] if p["id"] in metadata["workspace_members"]]
    source_groups = {p["name"]: {str(f.resolve()) for f in (Path(p["manifest_path"]).parent / "src").rglob("*.rs")} for p in packages}
    all_sources = set().union(*source_groups.values())
    module_index = ROOT / "crates/karate-api/src/lib.rs"
    if not all(re.fullmatch(r"\s*(?:#!\[forbid\(unsafe_code\)\]|pub mod [a-z_]+;)?\s*", line) for line in module_index.read_text().splitlines()):
        raise SystemExit("Module-index coverage exemption invalid: executable code was added")
    source_groups["karate-api"].remove(str(module_index.resolve()))
    compiler = Path(run(["rustup", "which", "--toolchain", args.toolchain, "rustc"], capture_output=True).stdout.strip())
    version = run([str(compiler), "-vV"], capture_output=True).stdout
    host = next(line.removeprefix("host: ") for line in version.splitlines() if line.startswith("host: "))
    llvm = compiler.parent.parent / "lib" / "rustlib" / host / "bin"
    for name in ("llvm-cov", "llvm-profdata"):
        if not (llvm / name).is_file():
            raise SystemExit("Install matching llvm-tools-preview for " + args.toolchain)
    evidence = ROOT / args.output_dir
    evidence.mkdir(parents=True, exist_ok=True)
    (ROOT / "target").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="coverage-", dir=ROOT / "target") as raw_dir:
        raw = Path(raw_dir)
        flags = "-C instrument-coverage" + (" -Z coverage-options=branch" if "nightly" in args.toolchain else "")
        env = dict(os.environ, RUSTFLAGS=flags, LLVM_PROFILE_FILE=str(raw / "%p-%m.profraw"),
                   CARGO_TARGET_DIR=str(ROOT / "target" / "coverage-build"), RUSTC=str(compiler),
                   RUSTDOC=str(compiler.parent / "rustdoc"), CARGO_BUILD_JOBS="2")
        build = subprocess.run([str(compiler.parent / "cargo"), "test", "--workspace", "--locked", "--offline", "--no-run", "--message-format=json"],
                               cwd=ROOT, text=True, env=env, capture_output=True)
        (evidence / "coverage-build.log").write_text(build.stderr)
        if build.returncode:
            raise SystemExit("Coverage build failed; see " + str(evidence / "coverage-build.log"))
        binaries, objects = set(), set()
        for line in build.stdout.splitlines():
            event = json.loads(line)
            if event.get("reason") == "compiler-artifact" and event.get("executable"):
                objects.add(event["executable"])
                if event.get("profile", {}).get("test"):
                    binaries.add(event["executable"])
        if not binaries:
            raise SystemExit("No test executables: coverage cannot pass")
        run(["./scripts/test-postgres.sh", "python3", "-c",
             "import subprocess,sys; [subprocess.run([p],check=True) for p in sys.argv[1:]]", *sorted(binaries)], env=env)
        profiles = sorted(raw.glob("*.profraw"))
        if not profiles:
            raise SystemExit("No coverage counters: coverage cannot pass")
        merged = raw / "merged.profdata"
        run([str(llvm / "llvm-profdata"), "merge", "-sparse", *map(str, profiles), "-o", str(merged)])
        object_args = [arg for binary in sorted(objects) for arg in ("-object", binary)]
        report = json.loads(run([str(llvm / "llvm-cov"), "export", *object_args,
            f"-instr-profile={merged}", "-skip-functions", "-ignore-filename-regex=(/tests/|/.cargo/|/rustc/)"], capture_output=True).stdout)
    expected = set().union(*source_groups.values())
    files = {entry["filename"]: entry for data in report["data"] for entry in data["files"]}
    if set(files) != expected:
        raise SystemExit(f"Coverage inventory mismatch: missing={expected-set(files)}, unexpected={set(files)-expected}")
    components = []
    for name, sources in sorted(source_groups.items()):
        entry = {"component": name}
        for metric in ("lines", "branches"):
            entry[metric] = {key: sum(files[path]["summary"][metric][key] for path in sources) for key in ("count", "covered")}
        components.append(entry)
        print(name, entry)
    summary = {"toolchain": version, "instrumentation_only": True, "production_toolchain": "1.99.1", "components": components,
               "files": [{"path": str(Path(path).relative_to(ROOT)), "sha256": hashlib.sha256(Path(path).read_bytes()).hexdigest()} for path in sorted(all_sources)],
               "lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
               "exclusions": ["Test sources", "Third-party/compiler code", "karate-api/src/lib.rs: module declarations only, enforced by source check; no executable logic"]}
    (evidence / "coverage-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    for entry in files.values():
        entry["filename"] = str(Path(entry["filename"]).relative_to(ROOT))
    (evidence / "coverage-llvm.json").write_text(json.dumps(report, separators=(",", ":")) + "\n")
    failures = []
    for component in components:
        for metric in ("lines", "branches") if args.require_branches else ("lines",):
            counts = component[metric]
            if not counts["count"] or counts["covered"] * 100 <= counts["count"] * 90:
                failures.append(component["component"] + " " + metric)
    if failures:
        raise SystemExit("FAIL: strictly greater than 90% required for " + ", ".join(failures))


if __name__ == "__main__":
    main()
