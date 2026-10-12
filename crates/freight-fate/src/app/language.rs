//! Which language the game speaks, and where its catalog comes from.
//!
//! The catalogs in the repository's `lang/` folder are compiled into the
//! executable, so every platform build carries every language with nothing
//! to package. A translator testing a correction drops their edited file in
//! the data folder's `lang/` (next to settings.json) and restarts: a file
//! there replaces the built-in catalog for that language. The translation
//! itself runs in `ff_core::l10n`; this module only picks and loads.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ff_core::l10n::{self, Catalog, Translator, ENGLISH};
use ff_core::models::profile::data_dir;

use super::GameContext;

/// Forces a language for one run, whatever the settings say: `es`, `en`.
/// The playtest harness and tests run in English unless this is set.
pub const LANG_ENV: &str = "FREIGHT_FATE_LANG";
/// A file every line the catalog does not translate is appended to, once
/// each: the to-do list a translator works from.
pub const MISSES_ENV: &str = "FREIGHT_FATE_L10N_MISSES";

macro_rules! catalogs {
    ($($code:literal),* $(,)?) => {
        &[$(($code, include_str!(concat!("../../../../lang/", $code, ".po")))),*]
    };
}

/// Every built-in catalog, by code.
const BUILT_IN: &[(&str, &str)] =
    catalogs!["es", "fr", "de", "pt_BR", "it", "ru", "pl", "zh_CN", "ja"];

/// A built-in catalog's text.
pub fn built_in_catalog(code: &str) -> Option<&'static str> {
    BUILT_IN
        .iter()
        .find(|(known, _)| *known == code)
        .map(|(_, text)| *text)
}

/// The data folder's override for `code`, when a translator put one there.
fn override_path(dir: &Path, code: &str) -> PathBuf {
    dir.join("lang").join(format!("{code}.po"))
}

/// The catalog text for `code`: the override in `dir` if there is one,
/// else the built-in copy.
fn catalog_text(dir: &Path, code: &str) -> Option<Cow<'static, str>> {
    let path = override_path(dir, code);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            log::info!("Using the translation file {}", path.display());
            return Some(Cow::Owned(text));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => log::warn!("Could not read {}: {err}", path.display()),
    }
    built_in_catalog(code).map(Cow::Borrowed)
}

/// How many lines a language's built-in catalog translates; a language
/// with none is not offered in Settings.
pub fn translated_count(code: &str) -> usize {
    if code == ENGLISH {
        return usize::MAX;
    }
    built_in_catalog(code)
        .and_then(|text| Catalog::from_po(code, text).ok())
        .map_or(0, |catalog| catalog.len())
}

/// The languages the Settings row cycles through: English, then every
/// language whose catalog translates something.
/// Worked out once: every catalog is compiled into the executable.
pub fn offered_languages() -> &'static [&'static l10n::Language] {
    static OFFERED: OnceLock<Vec<&'static l10n::Language>> = OnceLock::new();
    OFFERED.get_or_init(|| {
        l10n::LANGUAGES
            .iter()
            .filter(|lang| translated_count(lang.code) > 0)
            .collect()
    })
}

/// A translator for `code`, falling back to English when there is no
/// usable catalog.
pub fn load_translator(code: &str, dir: &Path) -> Translator {
    if code == ENGLISH {
        return Translator::english();
    }
    let Some(text) = catalog_text(dir, code) else {
        log::warn!("No translation for {code}; speaking English");
        return Translator::english();
    };
    let catalog = match Catalog::from_po(code, &text) {
        Ok(catalog) => catalog,
        Err(err) => {
            log::error!("The {code} translation could not be read ({err}); speaking English");
            return Translator::english();
        }
    };
    for problem in &catalog.problems {
        log::warn!("{code} translation, {problem}");
    }
    log::info!("Speaking {code}: {} translated lines", catalog.len());
    let translator = Translator::new(catalog);
    match std::env::var_os(MISSES_ENV).filter(|path| !path.is_empty()) {
        Some(path) => translator.log_misses_to(PathBuf::from(path)),
        None => translator,
    }
}

/// The operating system's language tag, e.g. "es-MX".
pub fn system_locale() -> Option<String> {
    sys_locale::get_locale()
}

impl GameContext {
    /// Load the catalog the language setting asks for (or `FREIGHT_FATE_LANG`
    /// when it is set). Called once at launch by the windowed game, and
    /// whenever the player changes the setting.
    pub fn apply_language(&mut self) {
        let wanted = std::env::var(LANG_ENV)
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| self.settings.language.clone());
        self.set_language(&wanted);
    }

    /// Speak `setting` (a code, or `auto`) from now on.
    pub fn set_language(&mut self, setting: &str) {
        let code = l10n::resolve(setting, system_locale().as_deref());
        if code != self.translator.language() {
            self.translator = load_translator(code, &data_dir());
        }
    }

    /// `text` in the player's language. Everything `say` and `say_event`
    /// speak already passes through here; a line that reaches the speech
    /// sink or message review another way calls it itself.
    pub fn tr(&self, text: &str) -> String {
        self.translator.translate(text).into_owned()
    }
}
