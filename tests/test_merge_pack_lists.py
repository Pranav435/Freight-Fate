"""The release publishes one pack list for every platform's update archive."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


def _load():
    spec = importlib.util.spec_from_file_location(
        "merge_pack_lists", ROOT / "tools" / "merge_pack_lists.py"
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


merge_pack_lists = _load()

LABEL = "1.9-tester-20261019"
PACKS = {
    "packs": [
        {"path": "freight_fate/music.pak", "sha256": "ab" * 32, "size": 3},
        {"path": "freight_fate/channel3000.pak", "sha256": "cd" * 32, "size": 4},
    ]
}


def write_lists(folder):
    for platform in merge_pack_lists.PLATFORMS:
        path = folder / f"FreightFate-{LABEL}-{platform}-packs.json"
        path.write_text(json.dumps(PACKS, indent=2), encoding="utf-8")


def test_four_matching_lists_become_one(tmp_path):
    # Windows writes its list with CRLF line endings; the content still matches.
    write_lists(tmp_path)
    (tmp_path / f"FreightFate-{LABEL}-windows-portable-packs.json").write_bytes(
        json.dumps(PACKS, indent=2).replace("\n", "\r\n").encode()
    )
    (tmp_path / f"FreightFate-{LABEL}-windows-portable.zip").write_bytes(b"zip")

    out = merge_pack_lists.merge(tmp_path)

    assert out.name == f"FreightFate-{LABEL}-packs.json"
    assert json.loads(out.read_text("utf-8")) == PACKS
    assert sorted(p.name for p in tmp_path.glob("*-packs.json")) == [out.name]
    assert (tmp_path / f"FreightFate-{LABEL}-windows-portable.zip").exists()


def test_lists_that_disagree_fail_the_release(tmp_path):
    write_lists(tmp_path)
    changed = {"packs": [dict(PACKS["packs"][0], size=9), PACKS["packs"][1]]}
    (tmp_path / f"FreightFate-{LABEL}-linux-arm64-packs.json").write_text(
        json.dumps(changed), encoding="utf-8"
    )
    with pytest.raises(RuntimeError, match="linux-arm64"):
        merge_pack_lists.merge(tmp_path)


def test_a_missing_platform_fails_the_release(tmp_path):
    write_lists(tmp_path)
    (tmp_path / f"FreightFate-{LABEL}-macos-arm64-packs.json").unlink()
    with pytest.raises(RuntimeError, match="macos-arm64"):
        merge_pack_lists.merge(tmp_path)
