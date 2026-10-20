"""Delete old Career 1.9 tester snapshot releases, keeping the newest few.

Only releases tagged ``1.9-tester-YYYYMMDD`` are considered: stable
releases, the ``nightly-*`` bridge for 1.8 players and anything else are
never touched. The git tags stay, because the snapshot workflow writes each
build's notes against the previous ``1.9-tester-*`` tag, and a kept tag
keeps the history.

Usage (needs ``gh`` and ``GH_TOKEN``): ``python3 tools/prune_snapshots.py --keep 7``.
Standard library only, so the release job runs it without ``uv``.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys

TESTER_TAG = re.compile(r"^1\.9-tester-\d{8}$")
DEFAULT_KEEP = 7


def releases_to_prune(tags: list[str], keep: int) -> list[str]:
    """The tester snapshot tags beyond the newest ``keep``, newest first."""
    if keep < 1:
        raise ValueError("keep must be at least 1")
    testers = sorted({tag for tag in tags if TESTER_TAG.match(tag)}, reverse=True)
    return testers[keep:]


def list_release_tags() -> list[str]:
    result = subprocess.run(
        ["gh", "release", "list", "--limit", "1000", "--json", "tagName"],
        check=True,
        capture_output=True,
        text=True,
    )
    return [release["tagName"] for release in json.loads(result.stdout)]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--keep", type=int, default=DEFAULT_KEEP)
    parser.add_argument("--dry-run", action="store_true", help="list, delete nothing")
    args = parser.parse_args(argv)
    doomed = releases_to_prune(list_release_tags(), args.keep)
    if not doomed:
        print(f"Nothing to prune; {args.keep} or fewer tester snapshots.")
        return 0
    failed = 0
    for tag in doomed:
        print(f"{'Would delete' if args.dry_run else 'Deleting'} release {tag} (tag kept)")
        if args.dry_run:
            continue
        if subprocess.run(["gh", "release", "delete", tag, "--yes"], check=False).returncode:
            failed += 1
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
