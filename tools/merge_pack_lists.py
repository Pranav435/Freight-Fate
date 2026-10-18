"""Merge each platform's pack list into the one list a release publishes.

Every build writes ``FreightFate-<label>-<platform>-packs.json`` beside its
update archive: the sound packs that archive leaves out, with their SHA-256.
The packs are the same files on every platform, so the release carries a
single ``FreightFate-<label>-packs.json`` instead of four identical ones,
and the in-game updater (``updater::slim``) looks for that name.

Usage: ``python3 tools/merge_pack_lists.py release-assets`` replaces the
per-platform lists in that folder with the merged one, and fails when the
lists disagree or carry more than one label. Standard library only, so the
release job runs it without ``uv``.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

# The builds whose update archives get a pack list.
PLATFORMS = ("windows-portable", "macos-arm64", "linux-x64", "linux-arm64")


def split_list_name(name: str) -> tuple[str, str] | None:
    """``FreightFate-L-linux-x64-packs.json`` -> ``("L", "linux-x64")``."""
    for platform in PLATFORMS:
        suffix = f"-{platform}-packs.json"
        if name.startswith("FreightFate-") and name.endswith(suffix):
            return name[len("FreightFate-") : -len(suffix)], platform
    return None


def merge(folder: Path) -> Path:
    """Write the merged list into ``folder``, delete the per-platform ones,
    and return the merged list's path."""
    found: dict[str, tuple[str, Path]] = {}
    for path in sorted(folder.glob("FreightFate-*-packs.json")):
        split = split_list_name(path.name)
        if split is not None:
            label, platform = split
            found[platform] = (label, path)
    missing = [platform for platform in PLATFORMS if platform not in found]
    if missing:
        raise RuntimeError(f"No pack list for: {', '.join(missing)}")
    labels = {label for label, _path in found.values()}
    if len(labels) != 1:
        raise RuntimeError(f"Pack lists carry more than one label: {', '.join(sorted(labels))}")
    lists = {
        platform: json.loads(path.read_text("utf-8")) for platform, (_l, path) in found.items()
    }
    first = lists[PLATFORMS[0]]
    differ = [platform for platform, packs in lists.items() if packs != first]
    if differ:
        raise RuntimeError(f"Pack lists differ from {PLATFORMS[0]}'s: {', '.join(differ)}")
    out = folder / f"FreightFate-{labels.pop()}-packs.json"
    out.write_text(json.dumps(first, indent=2) + "\n", encoding="utf-8")
    for _label, path in found.values():
        path.unlink()
    return out


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("folder", type=Path)
    args = parser.parse_args(argv)
    print(f"Wrote {merge(args.folder)}")


if __name__ == "__main__":
    main()
