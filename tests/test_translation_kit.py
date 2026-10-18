"""tools/translation_kit.py: the translator's kit as its own download."""

from __future__ import annotations

import importlib.util
import zipfile
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


def load_kit_module():
    path = ROOT / "tools" / "translation_kit.py"
    spec = importlib.util.spec_from_file_location("translation_kit", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_kit_carries_the_whole_lang_folder(tmp_path):
    kit = load_kit_module()
    path = kit.build_kit("1.9-tester-20261017", tmp_path)
    assert path.name == "FreightFate-1.9-tester-20261017-translation-kit.zip"
    with zipfile.ZipFile(path) as archive:
        names = set(archive.namelist())
    expected = {
        f"FreightFate-translation-kit/{p.relative_to(ROOT / 'lang').as_posix()}"
        for p in (ROOT / "lang").rglob("*")
        if p.is_file()
    }
    assert names == expected
    for required in ("README.md", "freight-fate.pot", "extra.pot", "es.po", "glossary/en.md"):
        assert f"FreightFate-translation-kit/{required}" in names


def test_kit_is_reproducible(tmp_path):
    kit = load_kit_module()
    first = kit.build_kit("x", tmp_path / "a").read_bytes()
    second = kit.build_kit("x", tmp_path / "b").read_bytes()
    assert first == second


def test_no_updater_suffix_matches_the_kit():
    kit = load_kit_module()
    name = kit.kit_name("1.9-tester-20261017")
    for suffix in (
        "-windows-portable.zip",
        "-macos.zip",
        "-macos-arm64.zip",
        "-linux-x64.tar.gz",
        "-linux-arm64.tar.gz",
        ".AppImage",
    ):
        assert not name.endswith(suffix)


def test_kit_refuses_a_folder_without_the_template(tmp_path):
    kit = load_kit_module()
    lang = tmp_path / "lang"
    lang.mkdir()
    (lang / "README.md").write_text("guide", encoding="utf-8")
    with pytest.raises(RuntimeError, match="template"):
        kit.build_kit("x", tmp_path / "out", lang=lang)
