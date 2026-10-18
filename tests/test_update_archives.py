"""Update archives: the release payload minus the big packs, plus the
manifest the in-game updater checks the player's copies against."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tarfile
import zipfile
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


def load_build_release_module():
    path = ROOT / "tools" / "build_release.py"
    spec = importlib.util.spec_from_file_location("build_release_updates", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def stage_payload(root: Path, build_release, *, windows: bool = False) -> None:
    """A staged FreightFate folder that verify_archive accepts."""
    files = {
        "FreightFate.exe" if windows else "FreightFate": b"executable",
        "build_info.json": b"{}",
        "LICENSE.txt": b"license",
        "USER_MANUAL.md": b"manual",
        "freight_fate/sounds.pak": b"sounds",
        "freight_fate/music.pak": b"music" * 100,
        "freight_fate/channel3000.pak": b"radio" * 50,
        build_release.RUST_BAKED_FILE_ENTRY: build_release.BAKED_MAGIC + bytes(8),
    }
    if windows:
        files.update({name: b"crt" for name in build_release.REQUIRED_CRT})
    for name, data in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    if not windows:
        (root / "FreightFate").chmod(0o755)


def test_names_derive_from_the_full_archive():
    build_release = load_build_release_module()
    full = "FreightFate-1.9-tester-20261018-windows-portable.zip"
    assert (
        build_release.update_archive_name(full)
        == "FreightFate-1.9-tester-20261018-windows-portable-update.zip"
    )
    assert (
        build_release.packs_manifest_name(full)
        == "FreightFate-1.9-tester-20261018-windows-portable-packs.json"
    )
    assert (
        build_release.update_archive_name("FreightFate-v1.9.1-linux-x64.tar.gz")
        == "FreightFate-v1.9.1-linux-x64-update.tar.gz"
    )


def test_no_updater_suffix_matches_an_update_asset():
    """An updater that predates update archives must never pick one."""
    build_release = load_build_release_module()
    suffixes = (
        "-windows-portable.zip",
        "-macos.zip",
        "-macos-arm64.zip",
        "-linux-x64.tar.gz",
        "-linux-arm64.tar.gz",
        "-linux-x86_64.AppImage",
        "-linux-aarch64.AppImage",
    )
    for full in (
        "FreightFate-x-windows-portable.zip",
        "FreightFate-x-macos-arm64.zip",
        "FreightFate-x-macos.zip",
        "FreightFate-x-linux-x64.tar.gz",
        "FreightFate-x-linux-arm64.tar.gz",
    ):
        for derived in (
            build_release.update_archive_name(full),
            build_release.packs_manifest_name(full),
        ):
            assert not derived.endswith(suffixes), derived


@pytest.mark.skipif(
    sys.platform == "win32", reason="Linux tarballs are built on Linux; Windows has no exec bit"
)
def test_linux_update_tarball_leaves_out_the_big_packs(tmp_path):
    build_release = load_build_release_module()
    build = tmp_path / "FreightFate"
    stage_payload(build, build_release)
    full = tmp_path / "FreightFate-1.9-tester-20261018-linux-x64.tar.gz"

    update = build_release.archive_update(build, full)

    assert update.name == "FreightFate-1.9-tester-20261018-linux-x64-update.tar.gz"
    build_release.verify_archive(update)
    with tarfile.open(update, "r:gz") as tar:
        names = set(tar.getnames())
    assert "FreightFate/freight_fate/sounds.pak" in names
    assert "FreightFate/freight_fate/music.pak" not in names
    assert "FreightFate/freight_fate/channel3000.pak" not in names
    # The staged build still has them for the full archive.
    assert (build / "freight_fate" / "music.pak").is_file()


def test_windows_update_zip_leaves_out_the_big_packs(tmp_path):
    build_release = load_build_release_module()
    build = tmp_path / "FreightFate"
    stage_payload(build, build_release, windows=True)
    full = tmp_path / "FreightFate-1.9-tester-20261018-windows-portable.zip"

    update = build_release.archive_update(build, full)

    build_release.verify_archive(update)
    with zipfile.ZipFile(update) as z:
        names = set(z.namelist())
    assert "FreightFate/FreightFate.exe" in names
    assert "FreightFate/freight_fate/music.pak" not in names
    assert "FreightFate/freight_fate/channel3000.pak" not in names


def test_update_verifier_rejects_an_update_that_carries_a_pack(tmp_path):
    build_release = load_build_release_module()
    build = tmp_path / "FreightFate"
    stage_payload(build, build_release)
    out = tmp_path / "FreightFate-x-linux-x64-update.tar.gz"

    def executable(member):
        if member.name == "FreightFate/FreightFate":
            member.mode = 0o755
        return member

    with tarfile.open(out, "w:gz") as tar:
        tar.add(build, arcname="FreightFate", filter=executable)
    with pytest.raises(RuntimeError, match="carries packs it should leave out"):
        build_release.verify_archive(out)


def test_manifest_and_build_info_name_each_pack_by_hash(tmp_path):
    build_release = load_build_release_module()
    build = tmp_path / "FreightFate"
    stage_payload(build, build_release)
    full = tmp_path / "FreightFate-x-linux-x64.tar.gz"

    manifest = json.loads(build_release.write_packs_manifest(build, full).read_text("utf-8"))
    build_release.stamp_build_info(build, "1.9-tester-20261018", build)
    info = json.loads((build / "build_info.json").read_text("utf-8"))

    expected = [
        {
            "path": relative,
            "sha256": hashlib.sha256((build / relative).read_bytes()).hexdigest(),
            "size": (build / relative).stat().st_size,
        }
        for relative in build_release.UPDATE_SKIPPED_PACKS
    ]
    assert manifest == {"packs": expected}
    assert info["packs"] == expected


def test_manifest_refuses_a_build_missing_a_pack(tmp_path):
    build_release = load_build_release_module()
    build = tmp_path / "FreightFate"
    stage_payload(build, build_release)
    (build / "freight_fate" / "channel3000.pak").unlink()
    with pytest.raises(RuntimeError, match="channel3000.pak"):
        build_release.write_packs_manifest(build, tmp_path / "FreightFate-x-linux-x64.tar.gz")
