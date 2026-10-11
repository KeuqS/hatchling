#!/usr/bin/env python3
"""Generate the seed-check fixture seed.

Independently reimplements the hash scheme ("Hash scheme
(normative)" in the mutation cache design doc) over
crates/hatchling-dev/seeds/fixture-tree/ and writes
crates/hatchling-dev/seeds/fixture-seed.json.

`hatchling-dev seed-check` (run by scripts/check.sh) verifies the Rust
implementation computes identical keys: any Python<->Rust scheme
divergence fails the gate. The fixture is FIXED input data — regenerate
only after a deliberate scheme change, never per work order.

Mirrors, in order:
- members(): dirs directly under <tree>/crates/ with a Cargo.toml,
  named by package.name, sorted by name.
- reverse_deps(): [dependencies] + [dev-dependencies] +
  [build-dependencies] edges, resolved by dep key, `package =` rename,
  or canonicalized `path =`; self-edges dropped; transitive closure.
- collect_files(): per directory, files first (name-sorted), then
  subdirectories (name-sorted), recursively; directory names
  target/.git/.mutation-cache/mutants.scratch/seeds or starting with
  mutants.out are skipped (files are never skipped).
- package_hash: per file `relpath + 0x00 + bytes + 0x00`, relpaths
  `/`-joined.
- suite_hash(P): over `sorted([P] + sorted(rdeps[P]))` (no dedup, like
  the Rust), per package `name + 0x00 + package_hash hex + 0x00`.
- lock_hash: SHA-256 of the fixture Cargo.lock bytes.
- env_id: pinned to HATCHLING_REQUIRE_CELL=1.

`toolchain_id` and `config_hash` are emitted empty: the check parses but
does not compare them (the toolchain legitimately varies across
machines; the config file does not exist in the fixture).
"""

import hashlib
import json
import os
import sys
import tomllib

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SEEDS_DIR = os.path.join(REPO_ROOT, "crates", "hatchling-dev", "seeds")
TREE = os.path.join(SEEDS_DIR, "fixture-tree")
OUT = os.path.join(SEEDS_DIR, "fixture-seed.json")
ENV_ID = "HATCHLING_REQUIRE_CELL=1"


def skipped_dir(name: str) -> bool:
    return name in (
        "target",
        ".git",
        ".mutation-cache",
        "mutants.scratch",
        "seeds",
    ) or name.startswith("mutants.out")


def collect_files(dirpath: str, rel_prefix: str = "") -> list:
    """(relpath, full_path); files first (sorted), then subdirs (sorted)."""
    entries = sorted(os.listdir(dirpath))
    files = [e for e in entries if os.path.isfile(os.path.join(dirpath, e))]
    dirs = [
        e
        for e in entries
        if os.path.isdir(os.path.join(dirpath, e)) and not skipped_dir(e)
    ]
    out = []
    for e in files:
        rel = f"{rel_prefix}/{e}" if rel_prefix else e
        out.append((rel, os.path.join(dirpath, e)))
    for e in dirs:
        rel = f"{rel_prefix}/{e}" if rel_prefix else e
        out.extend(collect_files(os.path.join(dirpath, e), rel))
    return out


def package_hash(member_dir: str) -> str:
    h = hashlib.sha256()
    for rel, full in collect_files(member_dir):
        with open(full, "rb") as f:
            data = f.read()
        h.update(rel.encode())
        h.update(b"\x00")
        h.update(data)
        h.update(b"\x00")
    return h.hexdigest()


def members(tree: str) -> dict:
    crates_dir = os.path.join(tree, "crates")
    out = {}
    for e in os.listdir(crates_dir):
        manifest = os.path.join(crates_dir, e, "Cargo.toml")
        if not os.path.isfile(manifest):
            continue
        with open(manifest, "rb") as f:
            name = tomllib.load(f)["package"]["name"]
        out[name] = os.path.join(crates_dir, e)
    return out


def resolve_dep_target(dep_key, spec, member_dirs, member_dir):
    if dep_key in member_dirs:
        return dep_key
    if not isinstance(spec, dict):
        return None
    if spec.get("package") in member_dirs:
        return spec["package"]
    if "path" in spec:
        resolved = os.path.normpath(os.path.join(member_dir, spec["path"]))
        for n, d in member_dirs.items():
            if os.path.normpath(d) == resolved:
                return n
    return None


def reverse_deps(tree: str, member_dirs: dict) -> dict:
    dependents = {name: set() for name in member_dirs}
    for name, mdir in member_dirs.items():
        with open(os.path.join(mdir, "Cargo.toml"), "rb") as f:
            table = tomllib.load(f)
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            for dep_key, spec in table.get(section, {}).items():
                target = resolve_dep_target(dep_key, spec, member_dirs, mdir)
                if target and target != name:
                    dependents[target].add(name)
    closure = {}
    for name in member_dirs:
        seen = set()
        stack = list(dependents[name])
        while stack:
            cur = stack.pop()
            if cur in seen:
                continue
            seen.add(cur)
            stack.extend(dependents[cur])
        closure[name] = seen
    return closure


def main() -> None:
    member_dirs = members(TREE)
    rdeps = reverse_deps(TREE, member_dirs)
    pkg_hashes = {n: package_hash(d) for n, d in member_dirs.items()}
    with open(os.path.join(TREE, "Cargo.lock"), "rb") as f:
        lock_hash = hashlib.sha256(f.read()).hexdigest()

    files = {}
    for name in sorted(member_dirs):
        mdir = member_dirs[name]
        for rel, full in collect_files(mdir):
            tree_rel = os.path.relpath(full, TREE).replace(os.sep, "/")
            with open(full, "rb") as f:
                file_hash = hashlib.sha256(f.read()).hexdigest()
            # No dedup: mirrors the Rust `sorted([pkg] + sorted(rdeps[pkg]))`.
            names = sorted([name] + sorted(rdeps[name]))
            h = hashlib.sha256()
            for n in names:
                h.update(n.encode())
                h.update(b"\x00")
                h.update(pkg_hashes[n].encode())
                h.update(b"\x00")
            files[tree_rel] = {
                "package": name,
                "key": {
                    "file_hash": file_hash,
                    "suite_hash": h.hexdigest(),
                    "lock_hash": lock_hash,
                    "toolchain_id": "",
                    "env_id": ENV_ID,
                    "config_hash": "",
                },
            }

    with open(OUT, "w") as f:
        json.dump({"files": files}, f, indent=2, sort_keys=True)
        f.write("\n")
    print(f"wrote {OUT} ({len(files)} files)")


if __name__ == "__main__":
    sys.exit(main())
