//! Smaller updates: keep the sound packs the player already has.
//!
//! Most of a release archive is two packs (`music.pak`, `channel3000.pak`)
//! that rarely change between builds. Each release therefore also carries,
//! beside every game archive, an update archive with those packs left out
//! (`X-windows-portable-update.zip`) and a manifest naming them with their
//! SHA-256 (`X-windows-portable-packs.json`), both cut by
//! `tools/build_release.py`. When the player's own copies match the
//! manifest, the updater downloads the update archive instead of the full
//! one.
//!
//! The update check decides. It trusts the install's `build_info.json`
//! (which records its packs' hashes) when the file sizes agree, so it reads
//! no pack; it hashes a pack only when the install predates that record.
//! The download only confirms each pack is still there at the same size,
//! because hashing half a gigabyte again there could not be cancelled and
//! would read as a stalled download. Any mismatch takes the full archive.
//!
//! Applying needs nothing new on Windows and Linux: their apply scripts
//! copy the update over the install folder, which keeps its packs. A Mac
//! update swaps the whole bundle, so [`complete_staged`] copies the
//! player's packs into the staged bundle first.

use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use super::{
    asset_digest, asset_named, asset_size, asset_url, resource_root_in, running_appimage_path,
    Platform, UpdateInfo, UpdaterEnv, USER_AGENT,
};
use crate::net::{self, NetError, Tier};

/// One pack an update archive leaves out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackRef {
    /// Relative to the payload root, `/`-separated: `freight_fate/music.pak`.
    pub path: String,
    /// Lowercase hex SHA-256.
    pub sha256: String,
    pub size: u64,
}

/// The smaller download a release offers alongside its full archive.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SlimUpdate {
    pub asset_name: String,
    pub asset_url: String,
    /// bytes
    pub asset_size: i64,
    /// Lowercase hex SHA-256 GitHub publishes for the asset; "" when unknown.
    pub asset_sha256: String,
    /// Where the pack manifest is published.
    pub manifest_url: String,
    /// The packs the update leaves out, from the manifest. Empty until the
    /// update check has read it and found the player's copies match.
    pub packs: Vec<PackRef>,
}

fn split_archive_name(full: &str) -> Option<(&str, &str)> {
    [".tar.gz", ".zip"]
        .into_iter()
        .find_map(|ext| full.strip_suffix(ext).map(|stem| (stem, ext)))
}

/// `X-windows-portable.zip` -> `X-windows-portable-update.zip`.
pub fn update_asset_name(full: &str) -> Option<String> {
    split_archive_name(full).map(|(stem, ext)| format!("{stem}-update{ext}"))
}

/// `X-windows-portable.zip` -> `X-windows-portable-packs.json`.
pub fn packs_manifest_name(full: &str) -> Option<String> {
    split_archive_name(full).map(|(stem, _)| format!("{stem}-packs.json"))
}

/// A release asset's download facts.
struct Asset {
    name: String,
    url: String,
    size: i64,
    sha256: String,
}

fn asset_by_name(release: &Value, name: &str) -> Option<Asset> {
    let asset = asset_named(release, name)?;
    Some(Asset {
        name: name.to_string(),
        url: asset_url(asset)?,
        size: asset_size(asset),
        sha256: asset_digest(asset),
    })
}

/// The update archive and manifest a release publishes for the full
/// archive `full_name`, when it publishes both.
pub(super) fn candidate(release: &Value, full_name: &str) -> Option<SlimUpdate> {
    let update = asset_by_name(release, &update_asset_name(full_name)?)?;
    let manifest = asset_by_name(release, &packs_manifest_name(full_name)?)?;
    Some(SlimUpdate {
        asset_name: update.name,
        asset_url: update.url,
        asset_size: update.size,
        asset_sha256: update.sha256,
        manifest_url: manifest.url,
        packs: Vec::new(),
    })
}

/// A safe relative path: plain names only, so a manifest can never point
/// outside the install.
fn relative_path(path: &str) -> Option<PathBuf> {
    let relative = PathBuf::from(path);
    let plain = !path.is_empty()
        && relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    plain.then_some(relative)
}

fn parse_pack(value: &Value) -> Option<PackRef> {
    let path = value.get("path")?.as_str()?;
    relative_path(path)?;
    let sha256 = value.get("sha256")?.as_str()?.to_ascii_lowercase();
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(PackRef {
        path: path.to_string(),
        sha256,
        size: value.get("size")?.as_u64()?,
    })
}

/// The packs a manifest (`{"packs": [...]}`) or `build_info.json` lists;
/// `None` when the list is missing or any entry is malformed.
pub fn parse_packs(value: &Value) -> Option<Vec<PackRef>> {
    value
        .get("packs")?
        .as_array()?
        .iter()
        .map(parse_pack)
        .collect::<Option<Vec<_>>>()
        .filter(|packs| !packs.is_empty())
}

/// Lowercase hex SHA-256 of the file at `path`.
pub fn file_sha256(path: &Path) -> io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Whether the install's copy of `pack` is the one the release expects.
/// With `recorded` (the install's own `build_info.json` packs), a matching
/// size and recorded hash are trusted without reading the pack.
fn installed_matches(root: &Path, pack: &PackRef, recorded: Option<&[PackRef]>) -> bool {
    if !installed_size_matches(root, pack) {
        return false;
    }
    let path = root.join(&pack.path);
    if recorded.is_some_and(|recorded| recorded.contains(pack)) {
        return true;
    }
    match file_sha256(&path) {
        Ok(digest) => digest == pack.sha256,
        Err(e) => {
            log::warn!("Could not read {}: {e}", path.display());
            false
        }
    }
}

/// Whether the install still has `pack` at its expected size.
fn installed_size_matches(root: &Path, pack: &PackRef) -> bool {
    relative_path(&pack.path).is_some_and(|relative| {
        fs::metadata(root.join(relative)).map(|m| m.len()).ok() == Some(pack.size)
    })
}

fn recorded_packs(root: &Path) -> Option<Vec<PackRef>> {
    let text = fs::read_to_string(root.join("build_info.json")).ok()?;
    parse_packs(&serde_json::from_str(&text).ok()?)
}

/// Keep `info.slim` only when its manifest reads and the install's packs
/// match it; `fetch` reads the manifest (injectable for tests).
pub fn resolve_with(
    info: &mut UpdateInfo,
    env: &UpdaterEnv,
    fetch: &dyn Fn(&str) -> Result<Value, NetError>,
) {
    let Some(slim) = info.slim.as_mut() else {
        return;
    };
    // An AppImage is one file: there is no folder of packs to keep.
    if running_appimage_path(env.appimage.as_deref()).is_some() {
        info.slim = None;
        return;
    }
    let packs = match fetch(&slim.manifest_url) {
        Ok(manifest) => parse_packs(&manifest),
        Err(e) => {
            log::warn!("Could not read the update's pack list: {e:?}");
            None
        }
    };
    let root = resource_root_in(env);
    let recorded = recorded_packs(&root);
    match packs {
        Some(packs)
            if packs
                .iter()
                .all(|pack| installed_matches(&root, pack, recorded.as_deref())) =>
        {
            log::info!(
                "Offering {} ({} bytes); this install's sound packs match",
                slim.asset_name,
                slim.asset_size
            );
            slim.packs = packs;
        }
        _ => {
            log::info!("Offering the full update; this install's sound packs differ");
            info.slim = None;
        }
    }
}

/// [`resolve_with`] over the network.
pub fn resolve(info: &mut UpdateInfo, env: &UpdaterEnv) {
    resolve_with(info, env, &|url| {
        let headers = vec![("User-Agent".to_string(), USER_AGENT.to_string())];
        net::request_json(Tier::GitHub, Some("GET"), url, None, &headers)
    });
}

/// What the download should fetch: the update archive when the install
/// still has the packs the update check matched, else the full archive. The second
/// value is the pack list [`complete_staged`] needs, `None` for a full
/// download.
pub fn choose_download(info: &UpdateInfo, env: &UpdaterEnv) -> (UpdateInfo, Option<Vec<PackRef>>) {
    let Some(slim) = info.slim.as_ref().filter(|slim| !slim.packs.is_empty()) else {
        return (info.clone(), None);
    };
    let root = resource_root_in(env);
    if !slim
        .packs
        .iter()
        .all(|pack| installed_size_matches(&root, pack))
    {
        log::warn!("A sound pack changed since the update check; downloading the full update");
        return (info.clone(), None);
    }
    let mut chosen = info.clone();
    chosen.asset_name = slim.asset_name.clone();
    chosen.asset_url = slim.asset_url.clone();
    chosen.asset_size = slim.asset_size;
    chosen.asset_sha256 = slim.asset_sha256.clone();
    chosen.slim = None;
    (chosen, Some(slim.packs.clone()))
}

/// Make the staged update whole before it is applied, and say whether it
/// now is. A Mac bundle is swapped whole, so the player's packs are copied
/// into it. On Windows and Linux the apply script copies over the install,
/// which keeps its packs, so nothing is copied and the staged folder stays
/// without them (`false`).
pub fn complete_staged(new_root: &Path, packs: &[PackRef], env: &UpdaterEnv) -> io::Result<bool> {
    if env.platform != Platform::MacOs || new_root.extension().is_none_or(|ext| ext != "app") {
        return Ok(packs.is_empty());
    }
    let installed = resource_root_in(env);
    let staged = new_root.join("Contents").join("Resources");
    for pack in packs {
        let relative = relative_path(&pack.path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bad pack path {}", pack.path),
            )
        })?;
        let target = staged.join(&relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(installed.join(&relative), &target)?;
    }
    Ok(true)
}
