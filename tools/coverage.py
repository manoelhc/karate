"""Collect real stable-Rust line coverage; never invent a branch denominator."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(args, **kwargs):
    return subprocess.run(args, cwd=ROOT, check=True, text=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--require-branches", action="store_true")
    args = parser.parse_args()
    metadata = json.loads(run(["./scripts/cargo.sh", "metadata", "--no-deps", "--locked", "--offline", "--format-version=1"], capture_output=True).stdout)
    if len(metadata["workspace_members"]) != 1:
        raise SystemExit("Add separate coverage gates for every new workspace component before proceeding")
    compiler = Path(run(["rustup", "which", "--toolchain", "1.99.1", "rustc"], capture_output=True).stdout.strip())
    version = run([str(compiler), "-vV"], capture_output=True).stdout
    host = next(line.removeprefix("host: ") for line in version.splitlines() if line.startswith("host: "))
    llvm = compiler.parent.parent / "lib" / "rustlib" / host / "bin"
    for name in ("llvm-cov", "llvm-profdata"):
        if not (llvm / name).is_file():
            raise SystemExit("Missing matching LLVM tools: rustup component add llvm-tools-preview --toolchain 1.99.1")
    (ROOT / "target").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="coverage-", dir=ROOT / "target") as raw_dir:
        raw = Path(raw_dir)
        env = dict(os.environ, RUSTFLAGS="-C instrument-coverage", LLVM_PROFILE_FILE=str(raw / "%p-%m.profraw"),
                   CARGO_TARGET_DIR=str(ROOT / "target" / "coverage-build"))
        result = run(["./scripts/cargo.sh", "test", "--workspace", "--locked", "--offline", "--no-run", "--message-format=json"],
                     env=env, capture_output=True)
        binaries = set()
        for line in result.stdout.splitlines():
            event = json.loads(line)
            if event.get("reason") == "compiler-artifact" and event.get("executable") and event.get("profile", {}).get("test"):
                binaries.add(event["executable"])
        if not binaries:
            raise SystemExit("No test executables: coverage cannot pass")
        for binary in sorted(binaries):
            run([binary], env=env)
        profiles = sorted(raw.glob("*.profraw"))
        if not profiles:
            raise SystemExit("No coverage counters: coverage cannot pass")
        merged = raw / "merged.profdata"
        run([str(llvm / "llvm-profdata"), "merge", "-sparse", *map(str, profiles), "-o", str(merged)])
        objects = [arg for binary in sorted(binaries) for arg in ("-object", binary)]
        report = json.loads(run([str(llvm / "llvm-cov"), "export", *objects,
            f"-instr-profile={merged}", "-skip-functions", "-ignore-filename-regex=(/tests/|/.cargo/|/rustc/)"], capture_output=True).stdout)
    expected = {str(path.resolve()) for path in (ROOT / "crates/karate-core/src").rglob("*.rs")}
    files = {entry["filename"]: entry for data in report["data"] for entry in data["files"]}
    if set(files) != expected:
        raise SystemExit(f"Coverage inventory mismatch: missing={expected-set(files)}, unexpected={set(files)-expected}")
    lines = sum(entry["summary"]["lines"]["count"] for entry in files.values())
    covered = sum(entry["summary"]["lines"]["covered"] for entry in files.values())
    branches = sum(entry["summary"]["branches"]["count"] for entry in files.values())
    branches_covered = sum(entry["summary"]["branches"]["covered"] for entry in files.values())
    summary = {"component": "karate-core", "toolchain": version, "lines": {"covered": covered, "total": lines},
               "branches": {"covered": branches_covered, "total": branches, "status": "measured" if branches else "blocked"},
               "files": {str(Path(path).relative_to(ROOT)): hashlib.sha256(Path(path).read_bytes()).hexdigest() for path in sorted(expected)},
               "lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
               "exclusions": ["Tests are not product executable logic", "Dependency and compiler sources are not first-party code"]}
    evidence = ROOT / "docs/specker/runs/2026-11-04-karate/evidence"
    (evidence / "coverage-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    # Store portable source filenames, without altering counters or denominators.
    for entry in files.values():
        entry["filename"] = str(Path(entry["filename"]).relative_to(ROOT))
    (evidence / "coverage-llvm.json").write_text(json.dumps(report, separators=(",", ":")) + "\n")
    print(f"karate-core lines: {covered}/{lines}; branches: {branches_covered}/{branches} ({summary['branches']['status']})")
    if not lines or covered * 100 <= lines * 90:
        raise SystemExit("FAIL: line coverage must be strictly greater than 90%")
    if args.require_branches and (not branches or branches_covered * 100 <= branches * 90):
        raise SystemExit("BLOCKED: stable coverage has no measured branch denominator; the full coverage gate cannot pass")


if __name__ == "__main__":
    main()
