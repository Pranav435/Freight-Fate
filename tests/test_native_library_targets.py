"""Which platforms the Rust build can actually get its native libraries on.

A Mac tester once found the port unbuildable for two reasons that no test
would have caught: the audio library fetcher bailed out on anything but
Windows, and the `sdl2` crate linked with a bare `-lSDL2` that never looks in
Homebrew's prefix, so the link failed with SDL2 sitting right there. Both are
build configuration rather than gameplay, and both are invisible from a
Windows machine -- which is exactly why they are pinned here rather than
left to whoever next opens a Mac. (The fetcher went with BASS; the audio
engine is now compiled in.)
"""

from __future__ import annotations

from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[1]


def test_cargo_supplies_current_cmake_compatibility_for_bundled_sdl2():
    """Source builders should not need a private Cargo workaround on macOS."""
    config = tomllib.loads((REPO_ROOT / ".cargo" / "config.toml").read_text(encoding="utf-8"))
    assert config["env"]["CMAKE_POLICY_VERSION_MINIMUM"] == {
        "value": "3.5",
        "force": False,
    }


def test_no_audio_library_is_fetched_or_shipped():
    """The audio engine is Hearken, compiled into the executable: there is
    no proprietary audio library to fetch per platform, and no add-on plugin
    folder to ship beside it."""
    assert not (REPO_ROOT / "tools" / "fetch_bass.py").exists()
    assert not (REPO_ROOT / "crates" / "bass-sys").exists()
    lib = REPO_ROOT / "assets" / "lib"
    assert not lib.exists() or not any(lib.glob("bass*"))


def test_linux_carries_sdl2_statically():
    """Linux compiles SDL2 in too: every distribution ships a different
    libSDL2-2.0.so.0, and a tarball that has to start on all of them cannot
    link against any one. The static build still opens X11, Wayland and the
    audio servers by dlopen, so nothing is lost but the dependency."""
    manifest = tomllib.loads(
        (REPO_ROOT / "crates" / "freight-fate" / "Cargo.toml").read_text(encoding="utf-8")
    )
    linux = manifest["target"]['cfg(target_os = "linux")']["dependencies"]["sdl2"]
    assert "bundled" in linux["features"]
    assert "static-link" in linux["features"]


def test_macos_carries_sdl2_statically():
    """macOS compiles SDL2 in; it must never link a system SDL again.

    Homebrew's `sdl2` became sdl2-compat -- a shim that loads SDL3 at
    RUNTIME, invisible to every install-name audit -- and the 2026-08-30
    tester zip died with "failed to load sdl3" on Macs without Homebrew.
    `bundled` + `static-link` builds the real SDL2 from source into the
    executable, so the shipped app depends on no SDL library at all.
    Windows must NOT get these features: the vendored import library is
    what the build script puts on the search path there.
    """
    manifest = tomllib.loads(
        (REPO_ROOT / "crates" / "freight-fate" / "Cargo.toml").read_text(encoding="utf-8")
    )
    macos = manifest["target"]['cfg(target_os = "macos")']["dependencies"]["sdl2"]
    assert "bundled" in macos["features"]
    assert "static-link" in macos["features"]
    assert "use-pkgconfig" not in macos["features"]

    windows = manifest["target"]["cfg(windows)"]["dependencies"]
    assert "sdl2" not in windows
    # The base dependency may carry platform-neutral features (the window
    # handle for the nonblocking Windows shutdown); the static-link set is
    # what must stay macOS-only.
    base = manifest["dependencies"]["sdl2"]
    assert base["workspace"] is True
    assert not {"bundled", "static-link", "use-pkgconfig"} & set(base.get("features", []))
