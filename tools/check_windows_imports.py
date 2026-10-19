"""Audit the Windows executable a Cargo build left, as the release does.

The release's payload check (``windows_runtime.verify_windows_runtime``)
refuses an executable that imports a DLL neither Windows nor the payload
provides, but it only runs in the snapshot build. This runs the same check
on the executable the PR CI's Windows test build already compiled, so a new
import fails the pull request that brings it in, not the next snapshot.

It stages what the release stages beside the executable (the executable
under its shipped name, the libraries the build scripts left in the profile
directory and Visual Studio's CRT) into a scratch folder and audits that.
Windows only: the CRT comes from the runner's Visual Studio.

    python tools/check_windows_imports.py [PROFILE_DIR]   # default target/debug
"""

from __future__ import annotations

import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from build_release import cargo_exe_name, rust_exe_name, rust_native_libraries  # noqa: E402
from windows_runtime import stage_windows_runtime  # noqa: E402


def check(profile_dir: Path) -> None:
    exe = profile_dir / cargo_exe_name("win32")
    if not exe.is_file():
        raise SystemExit(f"no executable at {exe}; build the game first")
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        shutil.copy2(exe, root / rust_exe_name("win32"))
        for lib in rust_native_libraries(profile_dir, {".dll"}):
            shutil.copy2(lib, root / lib.name)
        stage_windows_runtime(root)
    print(f"{exe} imports only DLLs Windows or the payload provides.")


if __name__ == "__main__":
    check(Path(sys.argv[1]) if len(sys.argv) > 1 else Path("target/debug"))
