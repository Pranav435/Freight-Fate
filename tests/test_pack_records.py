"""build_info.json records the sound packs an update can skip.

The in-game updater reads a zip release's index, compares each pack's size
and CRC-32 there with this record, and fetches the rest of the archive by
byte range when they match, so the update check reads no pack.
"""

from __future__ import annotations

import importlib.util
import json
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load_build_release_module():
    path = ROOT / "tools" / "build_release.py"
    spec = importlib.util.spec_from_file_location("build_release_packs", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_build_info_records_each_pack_by_size_and_crc32(tmp_path, monkeypatch):
    build_release = load_build_release_module()
    music = b"music" * 300_000
    radio = b"radio" * 50
    packs = tmp_path / "freight_fate"
    packs.mkdir()
    (packs / "music.pak").write_bytes(music)
    (packs / "channel3000.pak").write_bytes(radio)
    monkeypatch.setattr(build_release, "build_commit", lambda: "abc123")

    build_release.stamp_build_info(tmp_path, "1.9-tester-20261019", root=tmp_path)

    info = json.loads((tmp_path / "build_info.json").read_text("utf-8"))
    assert info["packs"] == [
        {"path": "freight_fate/music.pak", "size": len(music), "crc32": zlib.crc32(music)},
        {"path": "freight_fate/channel3000.pak", "size": len(radio), "crc32": zlib.crc32(radio)},
    ]


def test_a_build_without_packs_records_none(tmp_path, monkeypatch):
    build_release = load_build_release_module()
    monkeypatch.setattr(build_release, "build_commit", lambda: "abc123")
    build_release.stamp_build_info(tmp_path, "1.9-tester-20261019", root=tmp_path)
    assert "packs" not in json.loads((tmp_path / "build_info.json").read_text("utf-8"))
