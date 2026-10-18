"""Package the translator's kit as its own release download.

The kit is the repository's ``lang/`` folder: the template a new language
starts from, every language file to correct, the glossaries and the guide
(``README.md``, whose links are relative to the folder). The game reads the
catalogs compiled into it, so none of this ships inside the game archives;
a player who wants to translate downloads this one zip from the release page.

Usage: ``python3 tools/translation_kit.py --label 1.9-tester-20261017 --out release-assets``
writes ``release-assets/FreightFate-1.9-tester-20261017-translation-kit.zip``.
Standard library only, so the release job runs it without ``uv``.
"""

from __future__ import annotations

import argparse
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LANG = ROOT / "lang"
# The folder a player gets when they unzip the kit.
KIT_DIR = "FreightFate-translation-kit"
# Fixed timestamp, so the same lang/ folder always zips to the same bytes.
ZIP_DATE = (2026, 1, 1, 0, 0, 0)


def kit_name(label: str) -> str:
    """The kit's asset name. It ends in neither updater suffix
    (``-windows-portable.zip``, ``-macos.zip``, ...), so no updater picks it."""
    return f"FreightFate-{label}-translation-kit.zip"


def build_kit(label: str, out_dir: Path, lang: Path = LANG) -> Path:
    """Zip ``lang`` under ``KIT_DIR`` into ``out_dir``; return the zip's path."""
    template = lang / "freight-fate.pot"
    if not template.is_file():
        raise RuntimeError(f"Translation template was not found: {template}")
    if not (lang / "README.md").is_file():
        raise RuntimeError(f"Translator's guide was not found: {lang / 'README.md'}")
    out_dir.mkdir(parents=True, exist_ok=True)
    destination = out_dir / kit_name(label)
    files = sorted(path for path in lang.rglob("*") if path.is_file())
    with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in files:
            arcname = f"{KIT_DIR}/{path.relative_to(lang).as_posix()}"
            info = zipfile.ZipInfo(arcname, date_time=ZIP_DATE)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, path.read_bytes())
    return destination


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--label", required=True, help="release tag, e.g. 1.9-tester-20261017")
    parser.add_argument(
        "--out", type=Path, default=ROOT / "dist", help="folder to write the zip to"
    )
    args = parser.parse_args(argv)
    print(build_kit(args.label, args.out))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
