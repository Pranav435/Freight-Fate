"""Build Freight Fate for iPhone and iPad.

The iOS game is the desktop game: the same Rust states and menus, spoken
through Prism (which on iOS talks to VoiceOver, or to AVSpeech when VoiceOver
is off), with SDL2 running the UIKit window and game controllers and a small
Objective-C touch layer (``crates/freight-fate/ios/ff_touch.m``) turning
gestures into key presses. This script turns a ``cargo build`` for an iOS
target into an installable ``FreightFate.app``:

* the executable, with the baked world data, the committed loose sounds and
  the sound and music packs beside it (an iOS bundle is flat, so the game's
  "resources beside the executable" layout already fits);
* nothing under ``Frameworks/``: SDL2, Prism and the audio engine are
  all linked into the executable;
* an ``Info.plist`` declaring controller support, and the app icon compiled
  from ``crates/freight-fate/ios/Assets.xcassets``;
* a code signature: ad-hoc for the Simulator, or a real identity plus
  provisioning profile for a device.

Run from the repository root:

    uv run python tools/build_ios.py                  # Simulator build
    uv run python tools/build_ios.py --install --launch
    uv run python tools/build_ios.py --device \\
        --sign-identity "Apple Development: ..." \\
        --provisioning-profile path/to/profile.mobileprovision
    uv run python tools/build_ios.py --device --ipa --build-number 7 \\
        --sign-identity "Apple Distribution: ..." \\
        --provisioning-profile path/to/appstore.mobileprovision   # TestFlight

The Simulator build targets Apple Silicon Macs (``aarch64-apple-ios-sim``).
"""

from __future__ import annotations

import argparse
import importlib.util
import itertools
import os
import plistlib
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "build"
IOS_BUILD = BUILD / "ios"
APP_NAME = "FreightFate"
BUNDLE_ID = "net.orinks.freightfate"
ASSET_CATALOG = ROOT / "crates" / "freight-fate" / "ios" / "Assets.xcassets"
DISPLAY_NAME = "Freight Fate"
# SDL2's UIKit backend and Prism's VoiceOver backend both run on iOS 12,
# but GameController's extended-gamepad profile names and CoreHaptics need
# 14; nothing older is worth carrying.
MINIMUM_IOS = "14.0"


@dataclass(frozen=True)
class Target:
    triple: str
    sdk: str
    platform: str
    simulator: bool


SIMULATOR = Target("aarch64-apple-ios-sim", "iphonesimulator", "iPhoneSimulator", True)
DEVICE = Target("aarch64-apple-ios", "iphoneos", "iPhoneOS", False)


def load_build_release():
    """``build_release`` by path (tools is not a package)."""
    path = Path(__file__).resolve().parent / "build_release.py"
    sys.path.insert(0, str(path.parent))
    spec = importlib.util.spec_from_file_location("build_release", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def store_version(version: str) -> str:
    """``1.9.0.dev0`` -> ``1.9.0``: App Store Connect takes at most three integers."""
    parts = []
    for part in version.split(".")[:3]:
        digits = "".join(itertools.takewhile(str.isdigit, part))
        if not digits:
            break
        parts.append(digits)
    return ".".join(parts) or "0"


def info_plist(version: str, target: Target, build_number: str | None = None) -> dict:
    return {
        "CFBundleDevelopmentRegion": "en",
        "CFBundleDisplayName": DISPLAY_NAME,
        "CFBundleExecutable": APP_NAME,
        "CFBundleIdentifier": BUNDLE_ID,
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleName": APP_NAME,
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": store_version(version),
        "CFBundleVersion": build_number or version,
        "CFBundleSupportedPlatforms": [target.platform],
        "DTPlatformName": target.sdk,
        "LSRequiresIPhoneOS": True,
        "UIRequiredDeviceCapabilities": ["arm64"],
        "MinimumOSVersion": MINIMUM_IOS,
        "UIDeviceFamily": [1, 2],
        "UILaunchScreen": {},
        "UIRequiresFullScreen": True,
        "UIStatusBarHidden": True,
        "UIApplicationSupportsIndirectInputEvents": True,
        "UISupportedInterfaceOrientations": [
            "UIInterfaceOrientationPortrait",
            "UIInterfaceOrientationLandscapeLeft",
            "UIInterfaceOrientationLandscapeRight",
        ],
        "UISupportedInterfaceOrientations~ipad": [
            "UIInterfaceOrientationPortrait",
            "UIInterfaceOrientationPortraitUpsideDown",
            "UIInterfaceOrientationLandscapeLeft",
            "UIInterfaceOrientationLandscapeRight",
        ],
        # Game controllers: SDL2 reads them through GameController.
        "GCSupportsControllerUserInteraction": True,
        "GCSupportedGameControllers": [
            {"ProfileName": "ExtendedGamepad"},
            {"ProfileName": "MicroGamepad"},
        ],
        "NSBluetoothAlwaysUsageDescription": (
            "Freight Fate uses Bluetooth to talk to game controllers."
        ),
        "ITSAppUsesNonExemptEncryption": False,
    }


def sdk_info(target: Target) -> dict:
    """The toolchain keys Xcode stamps into Info.plist; App Store Connect checks them."""

    def run(*args: str) -> str:
        return subprocess.run(args, check=True, capture_output=True, text=True).stdout.strip()

    sdk_version = run("xcrun", "--sdk", target.sdk, "--show-sdk-version")
    xcode = run("xcodebuild", "-version").splitlines()
    xcode_version = xcode[0].split()[-1].split(".")
    xcode_build = xcode[-1].split()[-1]
    major = xcode_version[0]
    minor = xcode_version[1] if len(xcode_version) > 1 else "0"
    patch = xcode_version[2] if len(xcode_version) > 2 else "0"
    return {
        "DTCompiler": "com.apple.compilers.llvm.clang.1_0",
        "DTPlatformBuild": run("xcrun", "--sdk", target.sdk, "--show-sdk-build-version"),
        "DTPlatformVersion": sdk_version,
        "DTSDKBuild": run("xcrun", "--sdk", target.sdk, "--show-sdk-build-version"),
        "DTSDKName": f"{target.sdk}{sdk_version}",
        "DTXcode": f"{int(major):02d}{minor}{patch}",
        "DTXcodeBuild": xcode_build,
        "BuildMachineOSBuild": run("sw_vers", "-buildVersion"),
    }


def compile_icon(app: Path, target: Target) -> dict:
    """Compile the asset catalog into ``Assets.car``; return its Info.plist keys."""
    partial = IOS_BUILD / "assets-info.plist"
    subprocess.run(
        [
            "xcrun",
            "actool",
            "--compile",
            str(app),
            "--platform",
            target.sdk,
            "--minimum-deployment-target",
            MINIMUM_IOS,
            "--app-icon",
            "AppIcon",
            "--output-partial-info-plist",
            str(partial),
            "--target-device",
            "iphone",
            "--target-device",
            "ipad",
            "--output-format",
            "human-readable-text",
            str(ASSET_CATALOG),
        ],
        check=True,
        stdout=subprocess.DEVNULL,
    )
    with partial.open("rb") as f:
        return plistlib.load(f)


def cargo_build(target: Target, release: bool) -> Path:
    command = [
        "cargo",
        "build",
        "-p",
        "freight-fate",
        "--no-default-features",
        "--target",
        target.triple,
    ]
    if release:
        command.append("--release")
    env = dict(os.environ, IPHONEOS_DEPLOYMENT_TARGET=MINIMUM_IOS)
    subprocess.run(command, cwd=ROOT, check=True, env=env)
    return ROOT / "target" / target.triple / ("release" if release else "debug")


def provisioning_entitlements(profile: Path) -> dict:
    decoded = subprocess.run(
        ["security", "cms", "-D", "-i", str(profile)], check=True, capture_output=True
    ).stdout
    return plistlib.loads(decoded).get("Entitlements", {})


def codesign(app: Path, frameworks: list[Path], identity: str, entitlements: Path | None) -> None:
    for framework in frameworks:
        subprocess.run(
            ["codesign", "--force", "--sign", identity, "--timestamp=none", str(framework)],
            check=True,
        )
    command = ["codesign", "--force", "--sign", identity, "--timestamp=none"]
    if entitlements is not None:
        command += ["--entitlements", str(entitlements)]
    subprocess.run(command + [str(app)], check=True)
    subprocess.run(["codesign", "--verify", "--strict", "--deep", str(app)], check=True)


def stage_app(
    profile_dir: Path,
    target: Target,
    label: str,
    build_release,
    music: bool,
    build_number: str | None = None,
) -> tuple[Path, list[Path]]:
    app = IOS_BUILD / target.sdk / f"{APP_NAME}.app"
    if app.exists():
        shutil.rmtree(app)
    app.mkdir(parents=True)
    baked = build_release.bake_world_data(out=IOS_BUILD / build_release.RUST_BAKED_FILE)
    # The desktop layout plan, minus native libraries (there are none to
    # embed): executable, baked world, data files, loose sounds.
    plan = build_release.plan_rust_layout(
        profile_dir, platform_name="ios", native_exts={".framework"}, baked_data=baked
    )
    for source, relative in plan:
        destination = app / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    executable = app / APP_NAME
    executable.chmod(executable.stat().st_mode | 0o755)

    assets = build_release.PACKAGE_DIR / build_release.SOURCE_ASSETS
    build_release.require_real_pack(assets / "sounds.pak")
    shutil.copy2(assets / "sounds.pak", app / "freight_fate" / "sounds.pak")
    if music:
        build_release.ensure_music_pack()
        shutil.copy2(assets / "music.pak", app / "freight_fate" / "music.pak")
    credits = assets / "sounds" / "CREDITS.md"
    if credits.is_file():
        shutil.copy2(credits, app / "SOUND_CREDITS.md")
    build_release.stamp_build_info(app, label, root=app)

    # Nothing to embed: the audio engine is compiled in with everything else.
    frameworks: list[Path] = []
    info = info_plist(build_release.project_version(), target, build_number)
    info.update(compile_icon(app, target))
    if not target.simulator:
        info.update(sdk_info(target))
    with (app / "Info.plist").open("wb") as f:
        plistlib.dump(info, f)
    (app / "PkgInfo").write_text("APPL????", encoding="ascii")
    return app, frameworks


def package_ipa(app: Path) -> Path:
    """Zip the signed app as ``Payload/FreightFate.app`` for App Store Connect."""
    ipa = IOS_BUILD / f"{APP_NAME}.ipa"
    ipa.unlink(missing_ok=True)
    with tempfile.TemporaryDirectory() as staging:
        payload = Path(staging) / "Payload"
        payload.mkdir()
        subprocess.run(["ditto", str(app), str(payload / app.name)], check=True)
        subprocess.run(
            ["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(payload), str(ipa)],
            check=True,
        )
    return ipa


def simctl(*args: str) -> None:
    subprocess.run(["xcrun", "simctl", *args], check=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--device", action="store_true", help="build for a real iPhone or iPad")
    parser.add_argument("--debug", action="store_true", help="a debug build (default: release)")
    parser.add_argument("--tag", help="build label (default: the project version)")
    parser.add_argument(
        "--no-music", action="store_true", help="skip downloading and staging music.pak"
    )
    parser.add_argument(
        "--build-number",
        help="CFBundleVersion (default: the project version); must rise per upload",
    )
    parser.add_argument("--ipa", action="store_true", help="also package a device build as an .ipa")
    parser.add_argument("--sign-identity", help="codesign identity for a device build")
    parser.add_argument(
        "--provisioning-profile", type=Path, help="provisioning profile for a device build"
    )
    parser.add_argument(
        "--simulator",
        default="booted",
        help="Simulator to install into with --install (UDID or 'booted')",
    )
    parser.add_argument("--install", action="store_true", help="install into the Simulator")
    parser.add_argument("--launch", action="store_true", help="launch after installing")
    args = parser.parse_args(argv)

    target = DEVICE if args.device else SIMULATOR
    if args.device and not (args.sign_identity and args.provisioning_profile):
        parser.error("--device needs --sign-identity and --provisioning-profile")
    if args.device and (args.install or args.launch):
        parser.error("--install and --launch are for the Simulator")
    if args.ipa and not args.device:
        parser.error("--ipa is for a device build")

    build_release = load_build_release()
    label = args.tag or build_release.project_version()
    profile_dir = cargo_build(target, release=not args.debug)
    app, frameworks = stage_app(
        profile_dir, target, label, build_release, not args.no_music, args.build_number
    )

    entitlements = None
    identity = "-"
    if args.device:
        identity = args.sign_identity
        shutil.copy2(args.provisioning_profile, app / "embedded.mobileprovision")
        entitlements = IOS_BUILD / "entitlements.plist"
        with entitlements.open("wb") as f:
            plistlib.dump(provisioning_entitlements(args.provisioning_profile), f)
    codesign(app, frameworks, identity, entitlements)
    print(f"Built {app}")
    if args.ipa:
        print(f"Packaged {package_ipa(app)}")

    if args.install or args.launch:
        simctl("install", args.simulator, str(app))
    if args.launch:
        simctl("launch", args.simulator, BUNDLE_ID)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
