//! Translating the game's spoken text.
//!
//! The game says everything in English, built with `format!` at hundreds of
//! call sites. Rather than thread a translation call through every one of
//! them, each finished line is translated where it leaves for the speech
//! channels (`GameContext::say` and `say_event`). A catalog line is a
//! template: `It is {tod}.` matches "It is 3:15 PM." and hands `3:15 PM` to
//! the translation, `Es la hora: {tod}.` or however the language puts it.
//!
//! A line no template fills is split into sentences, then clauses, and each
//! piece is tried on its own. Whatever is still untranslated stays English,
//! so a missing or partial catalog never costs the player a line, only its
//! language. English itself is the identity: no catalog, no lookups.
//!
//! The catalogs are gettext PO files in the repository's `lang/` folder;
//! `lang/README.md` is the translator's guide. `tools/l10n/extract.py`
//! rebuilds the template list (`lang/freight-fate.pot`) from the source.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;

use aho_corasick::{AhoCorasick, MatchKind};
use parking_lot::Mutex;

pub mod matcher;
pub mod plural;
pub mod po;

use matcher::{hole_names, parse_source, parse_translation, Part, Seg, Template, TemplateSet};

/// The setting value that follows the operating system's language.
pub const AUTO: &str = "auto";
/// English: the language the game is written in.
pub const ENGLISH: &str = "en";

/// A language the game can speak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Language {
    /// The catalog's file stem and the settings value: `es`, `pt_BR`.
    pub code: &'static str,
    /// The language's name in that language, as the settings row reads it.
    pub autonym: &'static str,
}

/// Every language with a catalog, English first, then in settings order.
pub const LANGUAGES: &[Language] = &[
    Language {
        code: "en",
        autonym: "English",
    },
    Language {
        code: "es",
        autonym: "Español",
    },
    Language {
        code: "fr",
        autonym: "Français",
    },
    Language {
        code: "de",
        autonym: "Deutsch",
    },
    Language {
        code: "pt_BR",
        autonym: "Português do Brasil",
    },
    Language {
        code: "it",
        autonym: "Italiano",
    },
    Language {
        code: "ru",
        autonym: "Русский",
    },
    Language {
        code: "pl",
        autonym: "Polski",
    },
    Language {
        code: "zh_CN",
        autonym: "简体中文",
    },
    Language {
        code: "ja",
        autonym: "日本語",
    },
];

/// The language a code names, if the game has it.
pub fn language(code: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|lang| lang.code == code)
}

/// The catalog for an operating-system locale tag ("es-MX", "pt_BR.UTF-8",
/// "zh-Hans-CN"), or `None` when the game has no catalog for it.
///
/// Any Portuguese gets the Brazilian catalog and any Chinese the simplified
/// one: closer than English for the player, and a screen reader set to
/// either reads them.
pub fn match_locale(tag: &str) -> Option<&'static str> {
    let tag = tag.split(['.', '@']).next().unwrap_or(tag);
    let base = tag
        .split(['-', '_'])
        .next()
        .unwrap_or(tag)
        .to_ascii_lowercase();
    let code = match base.as_str() {
        "pt" => "pt_BR",
        "zh" => "zh_CN",
        other => other,
    };
    LANGUAGES
        .iter()
        .find(|lang| lang.code == code)
        .map(|lang| lang.code)
}

/// The catalog a language setting asks for: `auto` follows `system` (the
/// operating system's locale tag), an unknown value falls back to English.
pub fn resolve(setting: &str, system: Option<&str>) -> &'static str {
    if setting == AUTO || setting.is_empty() {
        return system.and_then(match_locale).unwrap_or(ENGLISH);
    }
    language(setting).map_or(ENGLISH, |lang| lang.code)
}

/// Placeholder names whose value is a place or a proper name: never sent
/// through the catalog, so "Mobile" stays a city in Alabama and never
/// becomes a phone.
const PROPER_NAME_HOLES: &[&str] = &[
    "city", "town", "village", "county", "state", "road", "street", "highway", "route", "exit",
    "where", "dest", "origin", "home", "station", "place", "artist", "song", "handle", "driver",
    "shipper", "receiver", "brand", "chain", "carrier", "company", "landmark",
];

/// The help's control names (`{{engine}}` in the source, `{control_engine}`
/// in the catalog): a key or button name, kept as the keyboard reads it.
const CONTROL_HOLE_PREFIXES: &[&str] = &["control_", "key_", "pad_"];

fn is_proper_name_hole(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    PROPER_NAME_HOLES.iter().any(|part| name.contains(part))
        || CONTROL_HOLE_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// How deep a line is split and its captures translated in turn.
const MAX_DEPTH: usize = 3;
/// Lines whose translation is remembered (the drive repeats itself).
const CACHE_LIMIT: usize = 8192;

/// One language's compiled catalog.
pub struct Catalog {
    language: String,
    exact: HashMap<String, String>,
    /// The multi-sentence exact entries, found inside longer lines.
    embedded: Option<AhoCorasick>,
    embedded_keys: Vec<String>,
    templates: TemplateSet,
    /// Entries that were rejected while compiling, for the checker.
    pub problems: Vec<String>,
}

impl Catalog {
    /// Compile a PO file's text. Untranslated (empty) and fuzzy entries are
    /// left out; a broken entry is recorded in `problems` and skipped, so
    /// one typo never costs the whole language.
    pub fn from_po(language: &str, text: &str) -> Result<Self, po::PoError> {
        let entries = po::parse(text)?;
        let mut exact = HashMap::new();
        let mut templates = Vec::new();
        let mut problems = Vec::new();
        for entry in entries {
            if entry.msgid.is_empty() || entry.msgstr.is_empty() || entry.is_fuzzy() {
                continue;
            }
            let segs = parse_source(&entry.msgid);
            let names = hole_names(&segs);
            if names.is_empty() {
                exact.insert(entry.msgid, entry.msgstr);
                continue;
            }
            let translation = parse_translation(&entry.msgstr);
            let unknown: Vec<&str> = translation
                .iter()
                .filter_map(|part| match part {
                    Part::Hole(name) | Part::Plural(name, _) => Some(name.as_str()),
                    Part::Lit(_) => None,
                })
                .filter(|name| !names.contains(name))
                .collect();
            if !unknown.is_empty() {
                problems.push(format!(
                    "line {}: the translation names {{{}}}, which the English line does not have",
                    entry.line,
                    unknown.join("}, {")
                ));
                continue;
            }
            match Template::new(segs, translation) {
                Ok(template) => templates.push(template),
                // A joiner like "{a}, {b}" is not an error; the splitter
                // already handles what it would.
                Err(_) => continue,
            }
        }
        // A fragment like "Billboard: " is also found by its bare words,
        // the way a piece reaches it after trimming.
        let trimmed: Vec<(String, String)> = exact
            .iter()
            .filter(|(key, _)| key.trim() != key.as_str() && !key.trim().is_empty())
            .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
            .collect();
        for (key, value) in trimmed {
            exact.entry(key).or_insert(value);
        }
        let mut embedded_keys: Vec<String> = exact
            .keys()
            .filter(|key| has_sentence_break(key))
            .cloned()
            .collect();
        embedded_keys.sort();
        let embedded = AhoCorasick::builder()
            .match_kind(MatchKind::LeftmostLongest)
            .build(&embedded_keys)
            .map_err(|err| log::error!("translations could not be indexed: {err}"))
            .ok();
        Ok(Self {
            language: language.to_string(),
            embedded,
            embedded_keys,
            exact,
            templates: TemplateSet::new(templates),
            problems,
        })
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    /// How many lines the catalog translates.
    pub fn len(&self) -> usize {
        self.exact.len() + self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Translates finished English lines into the player's language.
pub struct Translator {
    catalog: Option<Catalog>,
    cache: Mutex<HashMap<String, String>>,
    misses: Option<Mutex<MissLog>>,
}

/// Where untranslated lines are written, once each, for translators.
struct MissLog {
    path: PathBuf,
    seen: HashSet<String>,
}

impl Default for Translator {
    fn default() -> Self {
        Self::english()
    }
}

impl Translator {
    /// English: every line passes through untouched.
    pub fn english() -> Self {
        Self {
            catalog: None,
            cache: Mutex::new(HashMap::new()),
            misses: None,
        }
    }

    pub fn new(catalog: Catalog) -> Self {
        Self {
            catalog: Some(catalog),
            cache: Mutex::new(HashMap::new()),
            misses: None,
        }
    }

    /// Append every line that is not fully translated to `path`, once per
    /// session: the list a translator works from.
    pub fn log_misses_to(mut self, path: PathBuf) -> Self {
        self.misses = Some(Mutex::new(MissLog {
            path,
            seen: HashSet::new(),
        }));
        self
    }

    /// The catalog's language code, `en` for English.
    pub fn language(&self) -> &str {
        self.catalog.as_ref().map_or(ENGLISH, Catalog::language)
    }

    pub fn is_english(&self) -> bool {
        self.catalog.is_none()
    }

    /// The line in the player's language, as much of it as the catalog
    /// covers.
    pub fn translate<'a>(&self, line: &'a str) -> Cow<'a, str> {
        let Some(catalog) = &self.catalog else {
            return Cow::Borrowed(line);
        };
        if let Some(hit) = self.cache.lock().get(line) {
            return Cow::Owned(hit.clone());
        }
        let mut missed = Vec::new();
        let out = Pass {
            catalog,
            missed: &mut missed,
        }
        .line(line, 0)
        .unwrap_or_else(|| line.to_string());
        if let Some(log) = &self.misses {
            log.lock().record(&missed);
        }
        let mut cache = self.cache.lock();
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(line.to_string(), out.clone());
        Cow::Owned(out)
    }
}

impl MissLog {
    fn record(&mut self, missed: &[String]) {
        let fresh: Vec<&String> = missed
            .iter()
            .filter(|line| self.seen.insert((*line).clone()))
            .collect();
        if fresh.is_empty() {
            return;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path);
        match file {
            Ok(mut file) => {
                for line in fresh {
                    let _ = writeln!(file, "{}", line.replace('\n', "\\n"));
                }
            }
            Err(err) => log::warn!(
                "Could not write untranslated lines to {}: {err}",
                self.path.display()
            ),
        }
    }
}

/// One translation of one line: the catalog, and the pieces nothing
/// translated.
struct Pass<'c, 'm> {
    catalog: &'c Catalog,
    missed: &'m mut Vec<String>,
}

impl Pass<'_, '_> {
    /// A whole line or one of its pieces. `None` when nothing in it was
    /// translated; the piece is then recorded as missed.
    fn line(&mut self, text: &str, depth: usize) -> Option<String> {
        let out = self.piece(text, depth, true);
        if out.is_none() && has_words(text) {
            self.missed.push(text.trim().to_string());
        }
        out
    }

    fn piece(&mut self, text: &str, depth: usize, split: bool) -> Option<String> {
        let core = text.trim();
        if core.is_empty() {
            return None;
        }
        let lead = &text[..text.len() - text.trim_start().len()];
        let trail = &text[text.trim_end().len()..];
        let wrap = |inner: String| format!("{lead}{inner}{trail}");
        if let Some(out) = self.lookup(core, depth) {
            return Some(wrap(out));
        }
        // "Dispatch board." is the catalog's "Dispatch board" with the full
        // stop a menu adds after an item.
        // A lead-in like "Billboard:" keeps its colon the same way.
        if let Some(mark) = core
            .chars()
            .next_back()
            .filter(|c| matches!(c, '.' | ':' | ',' | ';'))
        {
            let bare = &core[..core.len() - mark.len_utf8()];
            if !bare.ends_with(['.', ':', ',', ';']) {
                let found = self.lookup(bare, depth);
                if let Some(mut out) = found {
                    if mark == '.' {
                        out = self.with_full_stop(out);
                    } else {
                        out.push(mark);
                    }
                    return Some(wrap(out));
                }
            }
        }
        if !split || depth >= MAX_DEPTH {
            return None;
        }
        if let Some(out) = self.embedded(core, depth) {
            return Some(wrap(out));
        }
        for splitter in [
            split_sentences as fn(&str) -> Vec<(&str, &str)>,
            split_clauses,
        ] {
            let pieces = splitter(core);
            if pieces.len() < 2 {
                continue;
            }
            let mut any = false;
            let mut out = String::new();
            let mut missed = Vec::new();
            for (piece, sep) in &pieces {
                let translated = {
                    let mut inner = Pass {
                        catalog: self.catalog,
                        missed: &mut missed,
                    };
                    inner.line(piece, depth + 1)
                };
                any |= translated.is_some();
                let piece_out = translated.unwrap_or_else(|| (*piece).to_string());
                let sep = if sep.trim().is_empty() && ends_full_width(&piece_out) {
                    ""
                } else {
                    sep
                };
                out.push_str(&piece_out);
                out.push_str(sep);
            }
            if any {
                self.missed.extend(missed);
                return Some(wrap(out));
            }
        }
        None
    }

    /// A line with a whole multi-sentence catalog entry inside it, such as a
    /// billboard after its lead-in: the entry is translated in place and
    /// the text around it piece by piece. Splitting into sentences first
    /// would break the entry into sentences the catalog does not have.
    fn embedded(&mut self, core: &str, depth: usize) -> Option<String> {
        let finder = self.catalog.embedded.as_ref()?;
        let mut out = String::new();
        let mut last = 0;
        let mut any = false;
        for found in finder.find_iter(core) {
            let (start, end) = (found.start(), found.end());
            let opens = start == 0 || core[..start].ends_with(char::is_whitespace);
            let closes = end == core.len()
                || core[end..].starts_with(|c: char| c.is_whitespace() || c.is_ascii_punctuation());
            if !opens || !closes {
                continue;
            }
            let gap = &core[last..start];
            if !gap.trim().is_empty() {
                let translated = self.line(gap, depth + 1);
                out.push_str(translated.as_deref().unwrap_or(gap));
            } else {
                out.push_str(gap);
            }
            let key = &self.catalog.embedded_keys[found.pattern().as_usize()];
            out.push_str(&self.catalog.exact[key]);
            last = end;
            any = true;
        }
        if !any {
            return None;
        }
        let gap = &core[last..];
        if !gap.trim().is_empty() {
            let translated = self.line(gap, depth + 1);
            out.push_str(translated.as_deref().unwrap_or(gap));
        } else {
            out.push_str(gap);
        }
        Some(out)
    }

    /// A translated piece with its sentence ended the way the language ends
    /// one, unless the translation already did.
    /// A whole-piece match: exact, then a template, then the same with the
    /// first letter's case flipped, since a fragment that opens a sentence
    /// ("Confirmed truck parking.") is the catalog's lowercase fragment.
    fn lookup(&mut self, core: &str, depth: usize) -> Option<String> {
        if let Some(hit) = self.catalog.exact.get(core) {
            return Some(hit.clone());
        }
        if let Some(out) = self.template(core, depth) {
            return Some(out);
        }
        let mut chars = core.chars();
        let first = chars.next()?;
        let upper = first.is_uppercase();
        let flipped: String = if upper {
            first.to_lowercase().chain(chars).collect()
        } else if first.is_lowercase() {
            first.to_uppercase().chain(chars).collect()
        } else {
            return None;
        };
        if let Some(hit) = self.catalog.exact.get(&flipped) {
            return Some(if upper { capitalize(hit) } else { hit.clone() });
        }
        // A fragment the game capitalised to open a sentence ("Divided, two
        // lanes your side") still fills its lowercase template.
        if upper {
            return self.template(&flipped, depth).map(|out| capitalize(&out));
        }
        None
    }

    fn with_full_stop(&self, mut out: String) -> String {
        if !out.trim_end().ends_with(['.', '!', '?', '。', '！', '？']) {
            let base = self.catalog.language.split('_').next().unwrap_or_default();
            out.push(if matches!(base, "zh" | "ja") {
                '。'
            } else {
                '.'
            });
        }
        out
    }

    /// The best template the line fills, rendered with its captures.
    fn template(&mut self, core: &str, depth: usize) -> Option<String> {
        // A one-sentence template never swallows several sentences into a
        // hole: "It is {tod}." must not match "It is 9:00. Fuel is low.",
        // which the sentence splitter translates whole.
        let (template, captures) = self.catalog.templates.best_match(core, |template, caps| {
            if template.numbers_only() && !caps.values().all(|value| is_number(value)) {
                return false;
            }
            has_sentence_break(&template.filled)
                || !caps.values().any(|value| has_sentence_break(value))
        })?;
        let mut out = String::new();
        for part in &template.translation {
            match part {
                Part::Lit(text) => out.push_str(text),
                Part::Hole(name) => {
                    let raw = captures.get(name.as_str()).copied().unwrap_or_default();
                    if is_proper_name_hole(name) || depth >= MAX_DEPTH {
                        // "{the_city}" holds "the Chicago" in English; the
                        // translation brings its own article, if any.
                        let bare = name
                            .starts_with("the_")
                            .then(|| raw.strip_prefix("the ").or(raw.strip_prefix("The ")))
                            .flatten();
                        out.push_str(bare.unwrap_or(raw));
                    } else if let Some(number) =
                        plural::localize_number(&self.catalog.language, raw)
                    {
                        out.push_str(&number);
                    } else {
                        // A capture is often a phrase the catalog knows
                        // ("heavy traffic"); an unknown one (a number, a
                        // name) is simply kept, and not a miss.
                        let mut ignored = Vec::new();
                        let translated = Pass {
                            catalog: self.catalog,
                            missed: &mut ignored,
                        }
                        .piece(raw, depth + 1, has_sentence_break(raw));
                        out.push_str(translated.as_deref().unwrap_or(raw));
                    }
                }
                Part::Plural(name, forms) => {
                    let raw = captures.get(name.as_str()).copied().unwrap_or_default();
                    out.push_str(plural::choose(&self.catalog.language, forms, raw));
                }
            }
        }
        // Every source hole must have been captured for the line to match;
        // a template whose segments carry a hole the translation drops is
        // still a faithful match (the translator chose to drop it).
        debug_assert!(template
            .segs
            .iter()
            .all(|seg| !matches!(seg, Seg::Hole(name) if !captures.contains_key(name.as_str()))));
        Some(out)
    }
}

/// `text` with its first letter capitalized.
fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A number as the game writes one: "3", "1,250", "2.5", "3:15".
fn is_number(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit())
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | ':' | '-' | ' '))
}

fn has_words(text: &str) -> bool {
    text.chars().filter(|c| c.is_alphabetic()).take(2).count() >= 2
}

fn has_sentence_break(text: &str) -> bool {
    split_sentences(text).len() > 1
}

/// Ends in a full-width sentence mark (Chinese, Japanese), after which no
/// space is written.
fn ends_full_width(text: &str) -> bool {
    text.trim_end()
        .ends_with(['。', '！', '？', '：', '；', '，', '、'])
}

/// Sentences, each with the whitespace that followed it: a break is a
/// `.`, `!` or `?` followed by a space and then a capital letter, a digit
/// or a quote.
fn split_sentences(text: &str) -> Vec<(&str, &str)> {
    split_at_marks(text, |text, at| {
        let before = text[..at].chars().next_back();
        if !matches!(before, Some('.' | '!' | '?')) {
            return None;
        }
        let rest = &text[at..];
        let gap = rest.len() - rest.trim_start().len();
        if gap == 0 {
            return None;
        }
        let next = rest[gap..].chars().next()?;
        (next.is_uppercase() || next.is_ascii_digit() || next == '"').then_some(gap)
    })
}

/// Clauses: split at ", ", ": ", "; " and " -- ", keeping the separator.
fn split_clauses(text: &str) -> Vec<(&str, &str)> {
    split_at_marks(text, |text, at| {
        let rest = &text[at..];
        for sep in [", ", ": ", "; ", " -- "] {
            if rest.starts_with(sep) {
                return Some(sep.len());
            }
        }
        None
    })
}

/// Walk `text`; `mark(text, at)` returns the separator length starting at
/// byte `at` when a piece ends there.
fn split_at_marks(text: &str, mark: impl Fn(&str, usize) -> Option<usize>) -> Vec<(&str, &str)> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < text.len() {
        if at > start {
            if let Some(len) = mark(text, at) {
                pieces.push((&text[start..at], &text[at..at + len]));
                at += len;
                start = at;
                continue;
            }
        }
        at += text[at..].chars().next().map_or(1, char::len_utf8);
    }
    if start < text.len() {
        pieces.push((&text[start..], ""));
    }
    pieces
}

#[cfg(test)]
mod tests;
