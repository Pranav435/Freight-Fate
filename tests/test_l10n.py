"""The translation tooling: extraction, the PO reader and writer, the checker."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools" / "l10n"))

import check  # noqa: E402
import extract  # noqa: E402
from po import Entry, parse_po, write_po  # noqa: E402
from rust_strings import is_player_text, literals_in, to_template  # noqa: E402


def test_language_files_match_the_source():
    # A spoken line changed in the code without re-running the extractor
    # leaves the translators working from a stale list.
    assert extract.main(["--check"]) == 0, "run: uv run python tools/l10n/extract.py"


def test_language_files_pass_the_checker():
    assert check.main([]) == 0


def test_format_strings_become_named_templates():
    src = 'fn f() { ctx.say(&format!("Truck damage is {} percent.", self.damage_pct(x))); }'
    (lit,) = literals_in(src)
    assert lit.is_format
    assert to_template(lit) == ("Truck damage is {damage_pct} percent.", ["damage_pct"])


def test_inline_names_and_specs():
    src = 'fn f() { format!("{city} in {:.1} miles, {city} again", d); }'
    (lit,) = literals_in(src)
    assert to_template(lit)[0] == "{city} in {d} miles, {city} again"


def test_logs_tests_and_keys_are_not_player_text():
    src = """
    fn f(x: &str) {
        log::warn!("Could not read the file");
        if x == "driving" {}
        match x { "quiet" => {}, _ => {} }
        let y = "sparse";
    }
    #[cfg(test)]
    mod tests { fn t() { let s = "Only in a test."; } }
    """
    kept = [
        lit.text for lit in literals_in(src) if is_player_text(lit.text, lit.is_format, lit.is_key)
    ]
    # The log line is filtered later by its macro; here only the key checks.
    assert "driving" not in kept
    assert "quiet" not in kept
    assert "sparse" in kept
    assert "Only in a test." not in kept


def test_po_round_trip():
    entries = [
        Entry(msgid="", msgstr="Language: es\n"),
        Entry(msgid='Say "hi"\nthere', msgstr='Di "hola"', comments=["placeholders: {x}"]),
        Entry(msgid="Gone", msgstr="Ido", obsolete=True),
    ]
    back = parse_po(write_po(entries, refs=False))
    assert [(e.msgid, e.msgstr, e.obsolete) for e in back] == [
        (e.msgid, e.msgstr, e.obsolete) for e in entries
    ]


def test_merge_keeps_translations_and_retires_removed_lines():
    template = [Entry(msgid="Kept"), Entry(msgid="New")]
    existing = [Entry(msgid="Kept", msgstr="Guardado"), Entry(msgid="Removed", msgstr="Quitado")]
    merged = extract.merge("es", template, existing)
    by_id = {e.msgid: e for e in merged}
    assert by_id["Kept"].msgstr == "Guardado"
    assert by_id["New"].msgstr == ""
    assert by_id["Removed"].obsolete


def test_checker_catches_what_the_game_would_lose():
    assert check.check_entry("es", "It is {tod}.", "Son las {tod}.") == []
    assert check.check_entry("es", "It is {tod}.", "Son las {hora}.") == [
        "names {hora}, which the English line does not have",
        "drops {tod}",
    ]
    assert check.check_entry("es", "{n} break{s_ending}", "{n} {n:pausa|pausas}") == []
    assert check.check_entry("es", "Exit 42 ahead.", "Salida adelante.") == ["loses the number 42"]
    assert check.check_entry("es", "Press Enter.", "Pulsa Intro.") == [
        "translates or drops the key name Enter"
    ]
    # A key name may touch Chinese or Japanese text with no space.
    assert check.check_entry("zh_CN", "Press Enter.", "按Enter。") == []
    assert check.check_entry("ru", "{n} miles", "{n} {n:миля|мили}") == [
        "{n:...} gives 2 forms; ru has 3"
    ]
