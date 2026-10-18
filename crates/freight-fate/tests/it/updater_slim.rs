//! Smaller updates: a release's update archive is offered only when the
//! install's sound packs match its manifest, checked again before the
//! download, and a Mac bundle gets the player's packs copied in.

use std::fs;
use std::path::{Path, PathBuf};

use freight_fate::net::NetError;
use freight_fate::updater::slim::{self, PackRef};
use freight_fate::updater::{self, Platform, UpdaterEnv};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FULL: &str = "FreightFate-v9.9.9-windows-portable.zip";
const UPDATE: &str = "FreightFate-v9.9.9-windows-portable-update.zip";
const MANIFEST: &str = "FreightFate-v9.9.9-windows-portable-packs.json";
const MANIFEST_URL: &str = "https://example.test/packs.json";

fn sha(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn asset(name: &str, size: i64) -> Value {
    json!({
        "name": name,
        "browser_download_url": if name == MANIFEST { MANIFEST_URL.to_string() } else { format!("https://example.test/{name}") },
        "size": size,
        "digest": format!("sha256:{}", "ab".repeat(32)),
    })
}

fn release(names: &[&str]) -> Value {
    let sizes = [(FULL, 531_000_000), (UPDATE, 35_000_000), (MANIFEST, 300)];
    json!({
        "tag_name": "v9.9.9",
        "body": "- **Something.**",
        "assets": names.iter().map(|name| {
            let size = sizes.iter().find(|(n, _)| n == name).map_or(1, |(_, s)| *s);
            asset(name, size)
        }).collect::<Vec<_>>(),
    })
}

const MUSIC: &[u8] = b"music pack bytes";
const RADIO: &[u8] = b"channel 3000 pack";

fn manifest(music: &[u8], radio: &[u8]) -> Value {
    json!({"packs": [
        {"path": "freight_fate/music.pak", "sha256": sha(music), "size": music.len()},
        {"path": "freight_fate/channel3000.pak", "sha256": sha(radio), "size": radio.len()},
    ]})
}

/// A packaged install: the executable, its packs, and optionally the
/// `build_info.json` packs record. Returns the executable's path.
fn install(root: &Path, music: &[u8], radio: &[u8], record: Option<Value>) -> PathBuf {
    let packs = root.join("freight_fate");
    fs::create_dir_all(&packs).unwrap();
    fs::write(packs.join("music.pak"), music).unwrap();
    fs::write(packs.join("channel3000.pak"), radio).unwrap();
    let mut info = json!({"tag": "v9.9.8", "channel": "stable"});
    if let Some(record) = record {
        info["packs"] = record["packs"].clone();
    }
    fs::write(root.join("build_info.json"), info.to_string()).unwrap();
    let exe = root.join("FreightFate.exe");
    fs::write(&exe, b"exe").unwrap();
    exe
}

fn offered(env: &UpdaterEnv, names: &[&str]) -> updater::UpdateInfo {
    updater::stable_update_from(&release(names), "9.9.8", env).expect("update offered")
}

fn serve(manifest: Value) -> impl Fn(&str) -> Result<Value, NetError> {
    move |url: &str| {
        assert_eq!(url, MANIFEST_URL);
        Ok(manifest.clone())
    }
}

#[test]
fn a_release_with_an_update_archive_and_manifest_offers_it() {
    let env = UpdaterEnv::fake(Platform::Windows, Path::new("/tmp/x/FreightFate.exe"));
    let info = offered(&env, &[FULL, UPDATE, MANIFEST]);
    assert_eq!(info.asset_name, FULL);
    let slim = info.slim.expect("update archive found");
    assert_eq!(slim.asset_name, UPDATE);
    assert_eq!(slim.asset_size, 35_000_000);
    assert_eq!(slim.manifest_url, MANIFEST_URL);
    assert!(
        slim.packs.is_empty(),
        "nothing is resolved before the manifest is read"
    );

    for names in [&[FULL][..], &[FULL, UPDATE], &[FULL, MANIFEST]] {
        assert_eq!(offered(&env, names).slim, None, "{names:?}");
    }
}

#[test]
fn matching_packs_make_the_update_archive_the_download() {
    let tmp = tempfile::tempdir().unwrap();
    let exe = install(tmp.path(), MUSIC, RADIO, None);
    let env = UpdaterEnv::fake(Platform::Windows, &exe);
    let mut info = offered(&env, &[FULL, UPDATE, MANIFEST]);
    assert_eq!(info.download_size(), 531_000_000);

    slim::resolve_with(&mut info, &env, &serve(manifest(MUSIC, RADIO)));

    assert_eq!(info.download_size(), 35_000_000);
    let (chosen, keep) = slim::choose_download(&info, &env);
    assert_eq!(chosen.asset_name, UPDATE);
    assert_eq!(chosen.asset_url, format!("https://example.test/{UPDATE}"));
    assert_eq!(chosen.asset_size, 35_000_000);
    assert_eq!(keep.map(|packs| packs.len()), Some(2));
}

#[test]
fn the_update_archive_size_reads_like_the_full_one() {
    let env = UpdaterEnv::fake(Platform::Windows, Path::new("/tmp/x/FreightFate.exe"));
    let mut listing = release(&[FULL, UPDATE, MANIFEST]);
    for asset in listing["assets"].as_array_mut().unwrap() {
        let size = asset["size"].as_i64().unwrap();
        asset["size"] = if asset["name"] == UPDATE {
            json!(size.to_string())
        } else {
            json!(size as f64)
        };
    }
    let info = updater::stable_update_from(&listing, "9.9.8", &env).expect("update offered");
    assert_eq!(info.asset_size, 531_000_000);
    assert_eq!(info.slim.map(|slim| slim.asset_size), Some(35_000_000));
}

#[test]
fn a_different_pack_keeps_the_full_download() {
    let tmp = tempfile::tempdir().unwrap();
    // Same sizes, different bytes: only the hash can tell.
    let exe = install(tmp.path(), b"MUSIC PACK BYTES", RADIO, None);
    let env = UpdaterEnv::fake(Platform::Windows, &exe);
    let mut info = offered(&env, &[FULL, UPDATE, MANIFEST]);
    slim::resolve_with(&mut info, &env, &serve(manifest(MUSIC, RADIO)));
    assert_eq!(info.slim, None);
    assert_eq!(info.download_size(), 531_000_000);

    let (chosen, keep) = slim::choose_download(&info, &env);
    assert_eq!(chosen.asset_name, FULL);
    assert!(keep.is_none());
}

#[test]
fn a_missing_pack_or_unreadable_manifest_keeps_the_full_download() {
    let tmp = tempfile::tempdir().unwrap();
    let exe = install(tmp.path(), MUSIC, RADIO, None);
    fs::remove_file(tmp.path().join("freight_fate/channel3000.pak")).unwrap();
    let env = UpdaterEnv::fake(Platform::Windows, &exe);
    let mut info = offered(&env, &[FULL, UPDATE, MANIFEST]);
    slim::resolve_with(&mut info, &env, &serve(manifest(MUSIC, RADIO)));
    assert_eq!(info.slim, None);

    let tmp = tempfile::tempdir().unwrap();
    let exe = install(tmp.path(), MUSIC, RADIO, None);
    let env = UpdaterEnv::fake(Platform::Windows, &exe);
    let mut info = offered(&env, &[FULL, UPDATE, MANIFEST]);
    slim::resolve_with(&mut info, &env, &|_url: &str| Err(NetError::http(404)));
    assert_eq!(info.slim, None);
}

#[test]
fn the_check_trusts_the_install_record_and_the_download_reads_no_pack() {
    let tmp = tempfile::tempdir().unwrap();
    // The install says it has the release's packs; the bytes on disk differ
    // at the same size, so a match here proves no pack was hashed.
    let exe = install(
        tmp.path(),
        b"MUSIC PACK BYTES",
        RADIO,
        Some(manifest(MUSIC, RADIO)),
    );
    let env = UpdaterEnv::fake(Platform::Windows, &exe);
    let mut info = offered(&env, &[FULL, UPDATE, MANIFEST]);
    slim::resolve_with(&mut info, &env, &serve(manifest(MUSIC, RADIO)));
    assert_eq!(info.download_size(), 35_000_000, "the check reads no pack");

    let (chosen, keep) = slim::choose_download(&info, &env);
    assert_eq!(chosen.asset_name, UPDATE, "the download hashes nothing");
    assert!(keep.is_some());
}

#[test]
fn a_pack_gone_since_the_check_takes_the_full_download() {
    let tmp = tempfile::tempdir().unwrap();
    let exe = install(tmp.path(), MUSIC, RADIO, None);
    let env = UpdaterEnv::fake(Platform::Windows, &exe);
    let mut info = offered(&env, &[FULL, UPDATE, MANIFEST]);
    slim::resolve_with(&mut info, &env, &serve(manifest(MUSIC, RADIO)));
    assert_eq!(info.download_size(), 35_000_000);

    fs::remove_file(tmp.path().join("freight_fate/music.pak")).unwrap();
    let (chosen, keep) = slim::choose_download(&info, &env);
    assert_eq!(chosen.asset_name, FULL);
    assert!(keep.is_none());
}

#[test]
fn an_appimage_run_never_takes_the_update_archive() {
    let tmp = tempfile::tempdir().unwrap();
    let exe = install(tmp.path(), MUSIC, RADIO, None);
    let mut env = UpdaterEnv::fake(Platform::Windows, &exe);
    let mut info = offered(&env, &[FULL, UPDATE, MANIFEST]);
    let appimage = tmp.path().join("FreightFate.AppImage");
    fs::write(&appimage, b"appimage").unwrap();
    env.appimage = Some(appimage.display().to_string());
    slim::resolve_with(&mut info, &env, &serve(manifest(MUSIC, RADIO)));
    assert_eq!(info.slim, None);
}

#[test]
fn a_manifest_cannot_point_outside_the_install() {
    for path in ["../music.pak", "/etc/passwd", "freight_fate/../../x", ""] {
        let manifest = json!({"packs": [{"path": path, "sha256": "ab".repeat(32), "size": 1}]});
        assert_eq!(slim::parse_packs(&manifest), None, "{path:?}");
    }
    let bad_hash =
        json!({"packs": [{"path": "freight_fate/music.pak", "sha256": "xyz", "size": 1}]});
    assert_eq!(slim::parse_packs(&bad_hash), None);
}

#[test]
fn names_follow_the_full_archive() {
    assert_eq!(slim::update_asset_name(FULL).as_deref(), Some(UPDATE));
    assert_eq!(slim::packs_manifest_name(FULL).as_deref(), Some(MANIFEST));
    assert_eq!(
        slim::update_asset_name("FreightFate-x-linux-x64.tar.gz").as_deref(),
        Some("FreightFate-x-linux-x64-update.tar.gz")
    );
    assert_eq!(
        slim::update_asset_name("FreightFate-x-linux-x86_64.AppImage"),
        None
    );
}

fn packs() -> Vec<PackRef> {
    slim::parse_packs(&manifest(MUSIC, RADIO)).unwrap()
}

#[test]
fn a_mac_update_bundle_gets_the_players_packs_copied_in() {
    let tmp = tempfile::tempdir().unwrap();
    let resources = tmp
        .path()
        .join("Applications/FreightFate.app/Contents/Resources");
    install(&resources, MUSIC, RADIO, None);
    let exe = tmp
        .path()
        .join("Applications/FreightFate.app/Contents/MacOS/FreightFate");
    fs::create_dir_all(exe.parent().unwrap()).unwrap();
    fs::write(&exe, b"exe").unwrap();
    let env = UpdaterEnv::fake(Platform::MacOs, &exe);
    let staged = tmp.path().join("staging/FreightFate.app");
    fs::create_dir_all(staged.join("Contents/Resources/freight_fate")).unwrap();

    assert!(slim::complete_staged(&staged, &packs(), &env).unwrap());

    let staged_packs = staged.join("Contents/Resources/freight_fate");
    assert_eq!(fs::read(staged_packs.join("music.pak")).unwrap(), MUSIC);
    assert_eq!(
        fs::read(staged_packs.join("channel3000.pak")).unwrap(),
        RADIO
    );
}

#[test]
fn a_folder_update_leaves_the_packs_where_they_are() {
    let tmp = tempfile::tempdir().unwrap();
    let exe = install(&tmp.path().join("game"), MUSIC, RADIO, None);
    let env = UpdaterEnv::fake(Platform::Windows, &exe);
    let staged = tmp.path().join("staging/FreightFate");
    fs::create_dir_all(&staged).unwrap();

    assert!(
        !slim::complete_staged(&staged, &packs(), &env).unwrap(),
        "the staged folder still leaves the packs out"
    );

    // The apply script copies over the install, which keeps its packs.
    assert!(!staged.join("freight_fate/music.pak").exists());
}
