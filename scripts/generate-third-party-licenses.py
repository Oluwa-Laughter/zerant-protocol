#!/usr/bin/env python3
"""Build a versioned third-party license inventory from pnpm and Cargo metadata.

Usage:
    python3 scripts/generate-third-party-licenses.py
    python3 scripts/generate-third-party-licenses.py --check

This records upstream declarations. It does not substitute for full license
texts, individual copyright notices, or a distribution compliance review.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "third-party-licenses.txt"



def run_metadata(command: list[str]) -> dict:
    result = subprocess.run(
        command, cwd=ROOT, capture_output=True, text=True, check=True
    )
    return json.loads(result.stdout)


def one_line(value: object) -> str:
    return str(value or "-").replace("\t", " ").replace("\r", " ").replace("\n", " ")


def local_notice_files(path: Path) -> str:
    if not path.is_dir():
        return "(not locally available)"
    names = sorted(
        file.name for file in path.iterdir()
        if file.is_file()
        and file.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE"))
    )
    return ", ".join(names) or "(not found in package root)"


def node_entries() -> list[tuple[str, str, str, str, str, str]]:
    report = run_metadata(["pnpm", "licenses", "list", "--json"])
    result = {}
    for declared, packages in report.items():
        for package in packages:
            name = str(package["name"])
            versions = package.get("versions") or []
            for version in versions:
                directory = Path((package.get("paths") or [""])[0])
                key = (name, str(version))
                license_id = declared or "UNRESOLVED"
                result[key] = (
                    name,
                    str(version),
                    license_id,
                    one_line(package.get("homepage")),
                    local_notice_files(directory),
                    one_line(package.get("author")),
                )
    return sorted(result.values(), key=lambda item: (item[0].lower(), item[1]))


def rust_entries() -> list[tuple[str, str, str, str, str, str]]:
    cargo = run_metadata(["cargo", "metadata", "--locked", "--format-version", "1"])
    result = {}
    for package in cargo["packages"]:
        if not package.get("source"):
            continue  # first-party workspace crate, governed by Zerant LICENSE
        name = str(package["name"])
        version = str(package["version"])
        license_id = one_line(package.get("license"))
        url = package.get("repository") or f"https://crates.io/crates/{name}/{version}"
        directory = Path(package["manifest_path"]).parent
        result[(name, version)] = (
            name,
            version,
            license_id,
            one_line(url),
            local_notice_files(directory),
            "-",
        )
    return sorted(result.values(), key=lambda item: (item[0].lower(), item[1]))


def render() -> str:
    js = node_entries()
    rust = rust_entries()
    pnpm_hash = hashlib.sha256((ROOT / "pnpm-lock.yaml").read_bytes()).hexdigest()
    cargo_hash = hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest()

    lines = [
        "ZERANT PROTOCOL — THIRD-PARTY DEPENDENCY LICENSE INVENTORY",
        "Generated from installed pnpm package metadata and locked Cargo metadata.",
        "Run: python3 scripts/generate-third-party-licenses.py",
        "",
        "This inventory is NOT a license for Zerant's proprietary source code.",
        "It records package metadata and locations of licenses/notices bundled with",
        "local dependencies. It is NOT a reproduction of every upstream copyright",
        "notice or complete license agreement; consult the original dependencies",
        "and THIRD_PARTY_NOTICES.md before redistributing compiled artifacts.",
        "Optional, platform-specific, test and build-only packages can be listed.",
        "",
        f"pnpm-lock.yaml SHA-256: {pnpm_hash}",
        f"Cargo.lock SHA-256:     {cargo_hash}",
        f"JavaScript packages (unique name/version): {len(js)}",
        f"Rust registry packages (unique name/version): {len(rust)}",
        "",
        "FORMAT: Name | Version | License metadata | Project/reference | Locally bundled notices | Author metadata",
        "",
        "=== JAVASCRIPT (PNPM DEPENDENCIES) ===",
    ]
    for entry in js:
        lines.append(" | ".join(one_line(value) for value in entry))
    lines += ["", "=== RUST (CARGO REGISTRY DEPENDENCIES) ==="]
    for entry in rust:
        lines.append(" | ".join(one_line(value) for value in entry))
    lines += ["", "END OF INVENTORY", ""]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="Fail if inventory is out of date")
    args = parser.parse_args()
    try:
        generated = render()
    except (subprocess.CalledProcessError, ValueError, FileNotFoundError) as exc:
        print(f"Cannot regenerate license inventory: {exc}", file=sys.stderr)
        return 1
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_text() != generated:
            print("License inventory is missing or does not match installed dependencies.")
            return 1
        print("License inventory is up to date.")
        return 0
    OUTPUT.write_text(generated, encoding="utf-8")
    print(f"Wrote {OUTPUT.name} ({len(generated.splitlines())} lines)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
