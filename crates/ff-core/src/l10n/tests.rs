use super::*;

const PO: &str = r#"
msgid ""
msgstr ""
"Language: es\n"

msgid "Settings"
msgstr "Ajustes"

msgid "off"
msgstr "desactivado"

msgid "heavy traffic"
msgstr "tráfico denso"

msgid "It is {tod}."
msgstr "Son las {tod}."

msgid "Truck damage is {damage_pct} percent."
msgstr "El camión tiene un {damage_pct} por ciento de daños."

msgid "{distance}: {condition} ahead."
msgstr "{distance}: {condition} más adelante."

msgid "Back in {where_}."
msgstr "De vuelta en {where_}."

msgid "{count} miles to go."
msgstr "Faltan {count} {count:milla|millas}."

#, fuzzy
msgid "Fuzzy line."
msgstr "No confirmada."

msgid "Untranslated line."
msgstr ""

msgid "Mobile"
msgstr "Móvil"

msgid "Bad hole {a}."
msgstr "Mal {b}."

msgid "Dispatch board"
msgstr "Tablón de cargas"

msgid "{label} {index} of {count}."
msgstr "{label} {index} de {count}."

msgid "{index} of {count}."
msgstr "{index} de {count}."

msgid "{prefix}{tons} tons of {cargo}"
msgstr "{prefix}{tons} toneladas de {cargo}"

msgid "Nuts. We sell them. Next exit."
msgstr "Nueces. Las vendemos. Próxima salida."

msgid "Billboard"
msgstr "Valla"

msgid "Multi"
"line."
msgstr "Varias "
"líneas."
"#;

fn spanish() -> Translator {
    Translator::new(Catalog::from_po("es", PO).expect("test catalog parses"))
}

#[test]
fn english_is_the_identity() {
    let tr = Translator::english();
    assert!(tr.is_english());
    assert!(matches!(tr.translate("It is 3:15 PM."), Cow::Borrowed(_)));
}

#[test]
fn exact_lines_translate() {
    let tr = spanish();
    assert_eq!(tr.translate("Settings"), "Ajustes");
    assert_eq!(tr.translate("Multiline."), "Varias líneas.");
}

#[test]
fn templates_carry_their_captures() {
    let tr = spanish();
    assert_eq!(tr.translate("It is 3:15 PM."), "Son las 3:15 PM.");
    assert_eq!(
        tr.translate("Truck damage is 12 percent."),
        "El camión tiene un 12 por ciento de daños."
    );
}

#[test]
fn captures_are_translated_when_the_catalog_knows_them() {
    let tr = spanish();
    assert_eq!(
        tr.translate("2 miles: heavy traffic ahead."),
        "2 miles: tráfico denso más adelante."
    );
}

#[test]
fn place_names_are_never_translated() {
    let tr = spanish();
    assert_eq!(tr.translate("Back in Mobile."), "De vuelta en Mobile.");
}

#[test]
fn plural_choices_follow_the_number() {
    let tr = spanish();
    assert_eq!(tr.translate("1 miles to go."), "Faltan 1 milla.");
    assert_eq!(tr.translate("3 miles to go."), "Faltan 3 millas.");
}

#[test]
fn sentences_translate_one_at_a_time() {
    let tr = spanish();
    assert_eq!(
        tr.translate("It is 9:00 AM. Truck damage is 3 percent."),
        "Son las 9:00 AM. El camión tiene un 3 por ciento de daños."
    );
    // An unknown sentence stays English beside a known one.
    assert_eq!(
        tr.translate("It is 9:00 AM. Nobody knows this one."),
        "Son las 9:00 AM. Nobody knows this one."
    );
}

#[test]
fn clauses_translate_one_at_a_time() {
    let tr = spanish();
    assert_eq!(tr.translate("Settings: off"), "Ajustes: desactivado");
}

#[test]
fn untranslated_fuzzy_and_broken_entries_are_skipped() {
    let catalog = Catalog::from_po("es", PO).expect("parses");
    assert_eq!(catalog.problems.len(), 1, "{:?}", catalog.problems);
    let tr = Translator::new(catalog);
    assert_eq!(tr.translate("Fuzzy line."), "Fuzzy line.");
    assert_eq!(tr.translate("Untranslated line."), "Untranslated line.");
    assert_eq!(tr.translate("Bad hole x."), "Bad hole x.");
}

#[test]
fn unknown_lines_pass_through_and_are_logged() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("misses.txt");
    let tr = spanish().log_misses_to(path.clone());
    assert_eq!(tr.translate("Nothing here."), "Nothing here.");
    assert_eq!(tr.translate("Nothing here."), "Nothing here.");
    assert_eq!(tr.translate("62"), "62");
    let logged = std::fs::read_to_string(path).expect("miss log written");
    assert_eq!(logged, "Nothing here.\n");
}

#[test]
fn non_ascii_text_survives_splitting() {
    let tr = Translator::new(
        Catalog::from_po(
            "zh_CN",
            "msgid \"It is {tod}.\"\nmsgstr \"现在是{tod}。\"\n\nmsgid \"Settings\"\nmsgstr \"设置\"\n",
        )
        .expect("parses"),
    );
    assert_eq!(
        tr.translate("It is 9:00 AM. Settings"),
        "现在是9:00 AM。设置"
    );
    // Already-translated text passes through without panicking on
    // multi-byte boundaries.
    assert_eq!(tr.translate("现在是。 设置: 你好"), "现在是。 设置: 你好");
}

#[test]
fn locales_map_to_catalogs() {
    assert_eq!(match_locale("es-MX"), Some("es"));
    assert_eq!(match_locale("pt_PT.UTF-8"), Some("pt_BR"));
    assert_eq!(match_locale("zh-Hans-CN"), Some("zh_CN"));
    assert_eq!(match_locale("en-US"), Some("en"));
    assert_eq!(match_locale("ko-KR"), None);
    assert_eq!(resolve(AUTO, Some("de-DE")), "de");
    assert_eq!(resolve(AUTO, None), "en");
    assert_eq!(resolve("fr", Some("de-DE")), "fr");
    assert_eq!(resolve("xx", None), "en");
}

#[test]
fn plural_rules() {
    use plural::plural_index;
    assert_eq!(plural_index("es", 1.0), 0);
    assert_eq!(plural_index("es", 2.0), 1);
    assert_eq!(plural_index("fr", 0.0), 0);
    assert_eq!(plural_index("ru", 21.0), 0);
    assert_eq!(plural_index("ru", 23.0), 1);
    assert_eq!(plural_index("ru", 25.0), 2);
    assert_eq!(plural_index("ru", 11.0), 2);
    assert_eq!(plural_index("pl", 1.0), 0);
    assert_eq!(plural_index("pl", 22.0), 1);
    assert_eq!(plural_index("pl", 21.0), 2);
    assert_eq!(plural_index("ja", 5.0), 0);
    assert_eq!(plural::number_in("1,250 dollars"), Some(1250.0));
    assert_eq!(plural::number_in("2.5 miles"), Some(2.5));
    assert_eq!(plural::number_in("none"), None);
}

#[test]
fn po_round_trips_escapes() {
    let text = "msgid \"a \\\"b\\\"\\n\"\nmsgstr \"c\\\\d\"\n";
    let entries = po::parse(text).expect("parses");
    assert_eq!(entries[0].msgid, "a \"b\"\n");
    assert_eq!(entries[0].msgstr, "c\\d");
    assert_eq!(po::quote("a \"b\"\n"), "\"a \\\"b\\\"\\n\"");
}

#[test]
fn menu_items_with_their_position_translate() {
    let tr = spanish();
    assert_eq!(
        tr.translate("Dispatch board. 3 of 16."),
        "Tablón de cargas. 3 de 16."
    );
    // A numbers-only template never takes words into its holes.
    assert_eq!(tr.translate("Out of fuel."), "Out of fuel.");
}

#[test]
fn an_empty_hole_and_side_by_side_holes_match() {
    let tr = spanish();
    assert_eq!(
        tr.translate("12 tons of heavy traffic"),
        "12 toneladas de tráfico denso"
    );
}

#[test]
fn a_known_passage_inside_a_longer_line_translates_whole() {
    let tr = spanish();
    assert_eq!(
        tr.translate("Billboard: Nuts. We sell them. Next exit. It is 3:15 PM."),
        "Valla: Nueces. Las vendemos. Próxima salida. Son las 3:15 PM."
    );
}

#[test]
fn a_captured_number_takes_the_languages_separators() {
    use super::plural::localize_number;
    assert_eq!(localize_number("de", "25,000").as_deref(), Some("25.000"));
    assert_eq!(localize_number("de", "4.89").as_deref(), Some("4,89"));
    assert_eq!(
        localize_number("fr", "1,250.5").as_deref(),
        Some("1\u{a0}250,5")
    );
    assert_eq!(localize_number("pt_BR", "-2.5").as_deref(), Some("-2,5"));
    // Plain integers, clock times, malformed groups and English-style
    // languages are left alone.
    assert_eq!(localize_number("de", "55"), None);
    assert_eq!(localize_number("de", "3:15"), None);
    assert_eq!(localize_number("de", "12,34"), None);
    assert_eq!(localize_number("ja", "25,000"), None);
    let catalog = Catalog::from_po(
        "de",
        "msgid \"{count} dollars\"\nmsgstr \"{count} Dollar\"\n",
    )
    .expect("parses");
    let tr = Translator::new(catalog);
    assert_eq!(tr.translate("25,000 dollars"), "25.000 Dollar");
}

#[test]
fn a_the_hole_drops_the_english_article() {
    let catalog = Catalog::from_po(
        "de",
        "msgid \"{the_city} metro freight market\"\nmsgstr \"Frachtmarkt Großraum {the_city}\"\n",
    )
    .expect("parses");
    let tr = Translator::new(catalog);
    assert_eq!(
        tr.translate("the Chicago metro freight market"),
        "Frachtmarkt Großraum Chicago"
    );
}

#[test]
fn a_fragment_opening_a_sentence_or_trimmed_still_matches() {
    let catalog = Catalog::from_po(
        "es",
        "msgid \"confirmed truck parking\"\nmsgstr \"aparcamiento confirmado\"\n\n\
         msgid \"Billboard: \"\nmsgstr \"Valla: \"\n",
    )
    .expect("parses");
    let tr = Translator::new(catalog);
    assert_eq!(
        tr.translate("Confirmed truck parking."),
        "Aparcamiento confirmado."
    );
    assert_eq!(tr.translate("Billboard:"), "Valla:");
}

#[test]
fn a_capitalised_fragment_still_fills_its_template() {
    let catalog = Catalog::from_po(
        "es",
        "msgid \"divided, {lane_word} lanes your side\"\n\
         msgstr \"calzadas separadas, {lane_word} vías en tu sentido\"\n\n\
         msgid \"{lane_word} lanes your side\"\nmsgstr \"{lane_word} vías en tu sentido\"\n\n\
         msgid \"two\"\nmsgstr \"dos\"\n",
    )
    .expect("parses");
    let tr = Translator::new(catalog);
    assert_eq!(
        tr.translate("Divided, two lanes your side."),
        "Calzadas separadas, dos vías en tu sentido."
    );
}

#[test]
fn side_by_side_holes_split_before_a_comma_tail() {
    let catalog = Catalog::from_po(
        "de",
        "msgid \"Merge onto {shield} toward {toward}{on_it_tail}.\"\n\
         msgstr \"Auf {shield} Richtung {toward} auffahren{on_it_tail}.\"\n\n\
         msgid \", then {distance} on it\"\nmsgstr \", dann {distance} darauf\"\n",
    )
    .expect("parses");
    let tr = Translator::new(catalog);
    assert_eq!(
        tr.translate("Merge onto I-90 East toward Rochester, New York, then 74 miles on it."),
        "Auf I-90 East Richtung Rochester, New York auffahren, dann 74 miles darauf."
    );
}
