#!/usr/bin/env python3
"""Check that every crate's integration tests stay in one test binary.

Each test binary links the whole dependency graph again, so a crate builds
its integration tests as ONE binary, `it` (tests/it/main.rs), and a new test
file goes in as a `mod` line there rather than a new top-level file. A few
tests need a process of their own and keep their own binary; they are listed
in SEPARATE_BINARIES with the reason, and CLAUDE.md names them too.

This fails when:

- a crate's tests/ holds a top-level test file or directory that is not `it`
  and not listed in SEPARATE_BINARIES (a new binary nobody decided on);
- a file or directory directly under tests/it/ is neither a `mod` of
  tests/it/main.rs nor pulled in by a `#[path]` attribute (a test that never
  compiles, so never runs).

Standard library only, so CI runs it without installing anything.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# crate -> {top-level entry under tests/: why it is its own binary}
SEPARATE_BINARIES = {
    "ff-core": {
        "data_baked.rs": "points the process at the baked container's data root",
        "data_map_correction.rs": "points the process at a corrected data root",
    },
    "freight-fate": {
        "classic_startup.rs": "needs a process where the 1.5 classics are not yet registered",
        "frame_time": "times frames against the wall clock, which sibling tests would contend for",
        "agent_server.rs": "agent tooling, `test = false`; CI runs it by name",
        "real_window.rs": "opens the real game window, `harness = false` and `test = false`; run by name on a desktop",
    },
}

PATH_ATTR = re.compile(r'#\[path\s*=\s*"([^"]+)"\]')
MOD = re.compile(r"^\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;", re.M)


def check_crate(crate: Path) -> list[str]:
    problems = []
    tests = crate / "tests"
    if not tests.is_dir():
        return problems
    allowed = SEPARATE_BINARIES.get(crate.name, {})
    for entry in sorted(tests.iterdir()):
        if entry.name == "it" or entry.name in allowed:
            continue
        if entry.is_file() and entry.suffix != ".rs":
            continue
        if entry.is_dir() and not (entry / "main.rs").exists():
            continue  # a fixture directory, not a test target
        rel = entry.relative_to(ROOT).as_posix()
        problems.append(
            f"{rel} is a test binary of its own; make it a `mod` of "
            f"{crate.name}/tests/it/main.rs, or list it in SEPARATE_BINARIES with the reason"
        )
    it = tests / "it"
    main = it / "main.rs"
    if it.is_dir():
        if not main.exists():
            problems.append(f"{it.relative_to(ROOT).as_posix()} has no main.rs")
            return problems
        listed = set(MOD.findall(main.read_text(encoding="utf-8")))
        # A sibling module may pull a file in with `#[path = "x.rs"]` to keep
        # its own file under the size limit; that file compiles too.
        for src in it.rglob("*.rs"):
            for target in PATH_ATTR.findall(src.read_text(encoding="utf-8")):
                listed.add(Path(target).stem)
        for entry in sorted(it.iterdir()):
            if entry.name == "main.rs":
                continue
            if entry.is_file() and entry.suffix != ".rs":
                continue
            name = entry.stem if entry.is_file() else entry.name
            if name not in listed:
                problems.append(
                    f"{entry.relative_to(ROOT).as_posix()} is not a `mod` of "
                    f"{main.relative_to(ROOT).as_posix()}, so it never compiles or runs"
                )
    return problems


def main() -> int:
    problems = []
    for toml in sorted(ROOT.glob("crates/*/Cargo.toml")):
        problems += check_crate(toml.parent)
    for p in problems:
        print(p)
    if problems:
        return 1
    print("test targets OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
