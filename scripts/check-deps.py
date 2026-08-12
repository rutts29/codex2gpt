#!/usr/bin/env python3
import argparse
import json
import sys
import tomllib
import urllib.request
from pathlib import Path


OSV_QUERY_BATCH_URL = "https://api.osv.dev/v1/querybatch"
CRATES_IO_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"


def parse_cargo_lock(path):
    data = tomllib.loads(Path(path).read_text())
    packages = []
    for package in data.get("package", []):
        if package.get("source") == CRATES_IO_SOURCE:
            packages.append((package["name"], package["version"]))
    return packages


def build_query_batch(packages):
    return {
        "queries": [
            {
                "package": {"name": name, "ecosystem": "crates.io"},
                "version": version,
            }
            for name, version in packages
        ]
    }


def query_osv_batch(packages, opener=urllib.request.urlopen, timeout=30):
    if not packages:
        return []

    body = json.dumps(build_query_batch(packages)).encode("utf-8")
    request = urllib.request.Request(
        OSV_QUERY_BATCH_URL,
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with opener(request, timeout=timeout) as response:
        data = json.loads(response.read().decode("utf-8"))

    findings = []
    for (name, version), result in zip(packages, data.get("results", [])):
        vulns = result.get("vulns", [])
        if vulns:
            findings.append(
                {
                    "package": name,
                    "version": version,
                    "vulnerabilities": vulns,
                }
            )
    return findings


def parse_args(argv):
    parser = argparse.ArgumentParser(description="Check Cargo.lock crates.io dependencies with OSV.")
    parser.add_argument("--lockfile", default="Cargo.lock", help="Path to Cargo.lock")
    parser.add_argument("--dry-run", action="store_true", help="Parse Cargo.lock without querying OSV")
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_args(argv or sys.argv[1:])
    packages = parse_cargo_lock(args.lockfile)
    findings = [] if args.dry_run else query_osv_batch(packages)
    print(json.dumps({"checked": len(packages), "findings": findings}, sort_keys=True))
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
