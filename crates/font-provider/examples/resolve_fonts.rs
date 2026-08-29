//! Prints the font chain `system_fonts()` resolves for every script, so the
//! platform provider can be checked against a real system font catalogue.
//!
//! `cargo run -p font-provider --example resolve_fonts`

use font_provider::system_fonts;
use translator::font_provider::FontRequest;
use translator::script::Script;

const SCRIPTS: [(Script, &str); 32] = [
    (Script::Latin, ""),
    (Script::Cyrillic, ""),
    (Script::Greek, ""),
    (Script::Armenian, ""),
    (Script::Hebrew, ""),
    (Script::Arabic, ""),
    (Script::Devanagari, ""),
    (Script::Bengali, ""),
    (Script::Gurmukhi, ""),
    (Script::Gujarati, ""),
    (Script::Oriya, ""),
    (Script::Tamil, ""),
    (Script::Telugu, ""),
    (Script::Kannada, ""),
    (Script::Malayalam, ""),
    (Script::Sinhala, ""),
    (Script::Thai, ""),
    (Script::Lao, ""),
    (Script::Tibetan, ""),
    (Script::Myanmar, ""),
    (Script::Georgian, ""),
    (Script::Ethiopic, ""),
    (Script::Khmer, ""),
    (Script::Han, "zh"),
    (Script::Han, "zh_hant"),
    (Script::Han, "zh-Hans"),
    (Script::Han, "zh-Hant"),
    (Script::Han, "ja"),
    (Script::Han, "ko"),
    (Script::Hiragana, ""),
    (Script::Katakana, ""),
    (Script::Hangul, ""),
];

fn main() {
    let fonts = system_fonts();

    for (script, language) in SCRIPTS {
        let request = FontRequest {
            script,
            language: language.to_string(),
            bold: false,
            italic: false,
            monospace: false,
        };
        report(&format!("{script:?} lang={language:?}"), &fonts, &request);
    }

    for (label, bold, italic, monospace) in [
        ("bold", true, false, false),
        ("italic", false, true, false),
        ("monospace", false, false, true),
    ] {
        let request = FontRequest {
            script: Script::Latin,
            language: "en".to_string(),
            bold,
            italic,
            monospace,
        };
        report(&format!("Latin {label}"), &fonts, &request);
    }
}

fn report(label: &str, fonts: &font_provider::SharedFontProvider, request: &FontRequest) {
    let chain = fonts.locate(request);
    if chain.is_empty() {
        println!("{label:<28} -> NOTHING");
        return;
    }
    for (i, handle) in chain.iter().enumerate() {
        let label = if i == 0 { label } else { "" };
        println!(
            "{label:<28} -> [{}] {} index={} weight={}",
            i,
            handle.path.display(),
            handle.ttc_index,
            handle.weight
        );
    }
}
