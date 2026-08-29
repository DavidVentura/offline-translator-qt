//! Turning a [`FontRequest`] into the DirectWrite fallback query that answers it.
//!
//! DirectWrite has no `fc-match`: `IDWriteFontFallback::MapCharacters` answers
//! "which family covers *this text* in *this locale*", one family per call. So
//! a `FontRequest` becomes a locale, a base family, a style, and an ordered
//! list of codepoints to ask about — the answers to which, in order, are the
//! chain the renderer walks.
//!
//! Nothing here touches COM, so it builds and its tests run on the development
//! host; `directwrite` is the only consumer.

use translator::font_provider::FontRequest;
use translator::script::Script;

/// A codepoint that stands in for a whole script when asking DirectWrite which
/// family covers it. `Script` is what the renderer itemizes by, and
/// `MapCharacters` only speaks in characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe(pub char);

/// A DirectWrite locale name (a BCP-47 tag). It is what makes Han resolve to
/// different families for `zh-Hans`, `ja-JP` and `ko-KR`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleName(pub String);

/// The family `MapCharacters` starts from. When it covers the probe the answer
/// is that family; otherwise DirectWrite consults its per-script fallback
/// tables. This is the closest thing DirectWrite has to fontconfig's
/// `sans-serif` / `monospace` generics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseFamily(pub &'static str);

/// Weight on the OpenType 100–900 scale, which is also DirectWrite's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontWeight(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle {
    Upright,
    Italic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackQuery {
    pub locale: LocaleName,
    pub base_family: BaseFamily,
    pub weight: FontWeight,
    pub style: FontStyle,
    /// Preference-ordered: the requested script first, then the generic tail.
    pub probes: Vec<Probe>,
}

/// The shell font on every supported Windows, and what DirectWrite's own
/// fallback tables are tuned around.
const UI_FAMILY: BaseFamily = BaseFamily("Segoe UI");

/// Cascadia Mono only ships with Windows 11 and the Terminal; Consolas has been
/// in the box since Vista.
const MONO_FAMILY: BaseFamily = BaseFamily("Consolas");

const REGULAR: FontWeight = FontWeight(400);
const BOLD: FontWeight = FontWeight(700);

/// Appended after the requested script's own probe. A run tagged with a script
/// also carries whatever `Common`/`Inherited` characters the itemizer folded
/// into it — digits, punctuation, symbols — which the script's family need not
/// cover, and those are the codepoints the renderer walks the chain for.
const GENERIC_PROBES: [Probe; 2] = [Probe('A'), Probe('★')];

pub fn plan(request: &FontRequest) -> FallbackQuery {
    let primary = representative_probe(request.script);
    let probes = std::iter::once(primary).chain(GENERIC_PROBES).fold(
        Vec::new(),
        |mut acc: Vec<Probe>, probe| {
            if !acc.contains(&probe) {
                acc.push(probe);
            }
            acc
        },
    );

    let base_family = if request.monospace {
        MONO_FAMILY
    } else {
        UI_FAMILY
    };

    FallbackQuery {
        locale: locale_name(request.script, &request.language),
        base_family,
        weight: if request.bold { BOLD } else { REGULAR },
        style: if request.italic {
            FontStyle::Italic
        } else {
            FontStyle::Upright
        },
        probes,
    }
}

/// A codepoint whose Unicode script property *is* `script`, so DirectWrite's
/// fallback resolves the family it uses for that script. Every one is the first
/// letter of the script's alphabet or syllabary, which no family ships without
/// while claiming to support the script.
fn representative_probe(script: Script) -> Probe {
    let ch = match script {
        Script::Latin => 'A',
        Script::Cyrillic => 'А',
        Script::Greek => 'Α',
        Script::Armenian => 'Ա',
        Script::Hebrew => 'א',
        Script::Arabic => 'ا',
        Script::Devanagari => 'अ',
        Script::Bengali => 'অ',
        Script::Gurmukhi => 'ਅ',
        Script::Gujarati => 'અ',
        Script::Oriya => 'ଅ',
        Script::Tamil => 'அ',
        Script::Telugu => 'అ',
        Script::Kannada => 'ಅ',
        Script::Malayalam => 'അ',
        Script::Sinhala => 'අ',
        Script::Thai => 'ก',
        Script::Lao => 'ກ',
        Script::Tibetan => 'ཀ',
        Script::Myanmar => 'က',
        Script::Georgian => 'ა',
        Script::Ethiopic => 'ሀ',
        Script::Khmer => 'ក',
        Script::Han => '一',
        Script::Hiragana => 'あ',
        Script::Katakana => 'ア',
        Script::Hangul => '가',
        // Itemization categories, not writing systems: a run of these reached
        // the provider with no strong neighbour to inherit from, so it is
        // digits and punctuation and the UI font is the right answer.
        Script::Common | Script::Inherited | Script::Other => 'A',
    };
    Probe(ch)
}

/// The caller's BCP-47 hint when it carries information, otherwise a
/// script-default. Unlike the fontconfig provider the whole tag is kept rather
/// than the primary subtag: DirectWrite reads the script subtag, so `zh-Hant`
/// picks a different family from `zh-Hans`.
fn locale_name(script: Script, language: &str) -> LocaleName {
    let hint = canonical_tag(&language.replace('_', "-"));
    let default = script_locale(script);
    let primary = hint.split('-').next().unwrap_or("");

    if primary.is_empty() || primary == "und" {
        return LocaleName(default.to_string());
    }

    // A bare `zh` does not tell DirectWrite which Han variant is meant, so it
    // answers from the user's language list — a Japanese face on a Japanese or
    // English system, which then has no glyph for the simplified-only
    // characters. The script default names the variant, so prefer it whenever
    // the hint agrees on the language and adds nothing else.
    let unqualified = !hint.contains('-');
    if unqualified
        && primary
            == default
                .split('-')
                .next()
                .expect("defaults carry a language")
    {
        return LocaleName(default.to_string());
    }

    LocaleName(hint)
}

/// The locale a script means with no hint from the caller. Each is fully
/// qualified, since that is what makes it an answer rather than a question.
fn script_locale(script: Script) -> &'static str {
    match script {
        Script::Latin => "en-US",
        Script::Cyrillic => "ru-RU",
        Script::Greek => "el-GR",
        Script::Armenian => "hy-AM",
        Script::Hebrew => "he-IL",
        Script::Arabic => "ar-SA",
        Script::Devanagari => "hi-IN",
        Script::Bengali => "bn-IN",
        Script::Gurmukhi => "pa-IN",
        Script::Gujarati => "gu-IN",
        Script::Oriya => "or-IN",
        Script::Tamil => "ta-IN",
        Script::Telugu => "te-IN",
        Script::Kannada => "kn-IN",
        Script::Malayalam => "ml-IN",
        Script::Sinhala => "si-LK",
        Script::Thai => "th-TH",
        Script::Lao => "lo-LA",
        Script::Tibetan => "bo-CN",
        Script::Myanmar => "my-MM",
        Script::Georgian => "ka-GE",
        Script::Ethiopic => "am-ET",
        Script::Khmer => "km-KH",
        Script::Han => "zh-Hans",
        Script::Hiragana | Script::Katakana => "ja-JP",
        Script::Hangul => "ko-KR",
        Script::Common | Script::Inherited | Script::Other => "en-US",
    }
}

/// BCP-47 canonical case — language lowercase, script title case, region upper
/// case. The catalog spells Traditional Chinese `zh_hant`, and a locale tag is
/// matched by its shape, so the shape is what gets normalised here.
fn canonical_tag(tag: &str) -> String {
    let subtags: Vec<String> = tag
        .split('-')
        .enumerate()
        .map(|(i, sub)| match (i, sub.len()) {
            (0, _) => sub.to_lowercase(),
            (_, 4) => {
                let (head, rest) = sub.split_at(1);
                format!("{}{}", head.to_uppercase(), rest.to_lowercase())
            }
            (_, 2) | (_, 3) => sub.to_uppercase(),
            _ => sub.to_lowercase(),
        })
        .collect();
    subtags.join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_SCRIPTS: [Script; 30] = [
        Script::Latin,
        Script::Cyrillic,
        Script::Greek,
        Script::Armenian,
        Script::Hebrew,
        Script::Arabic,
        Script::Devanagari,
        Script::Bengali,
        Script::Gurmukhi,
        Script::Gujarati,
        Script::Oriya,
        Script::Tamil,
        Script::Telugu,
        Script::Kannada,
        Script::Malayalam,
        Script::Sinhala,
        Script::Thai,
        Script::Lao,
        Script::Tibetan,
        Script::Myanmar,
        Script::Georgian,
        Script::Ethiopic,
        Script::Khmer,
        Script::Han,
        Script::Hiragana,
        Script::Katakana,
        Script::Hangul,
        Script::Common,
        Script::Inherited,
        Script::Other,
    ];

    fn request(script: Script) -> FontRequest {
        FontRequest {
            script,
            language: String::new(),
            bold: false,
            italic: false,
            monospace: false,
        }
    }

    /// The whole design rests on the probe actually belonging to the script it
    /// stands for — a probe from the wrong block would silently resolve the
    /// wrong family. `Script::of_char` is the same classifier the renderer
    /// itemizes with, so this checks the round trip.
    #[test]
    fn every_probe_belongs_to_the_script_it_represents() {
        for script in ALL_SCRIPTS {
            let Probe(ch) = representative_probe(script);
            let expected = match script {
                Script::Common | Script::Inherited | Script::Other => Script::Latin,
                other => other,
            };
            assert_eq!(Script::of_char(ch), expected, "probe {ch:?} for {script:?}");
        }
    }

    #[test]
    fn generic_probes_are_latin_and_symbol() {
        assert_eq!(Script::of_char('A'), Script::Latin);
        assert_eq!(Script::of_char('★'), Script::Common);
    }

    #[test]
    fn chain_leads_with_the_requested_script_and_never_repeats() {
        for script in ALL_SCRIPTS {
            let query = plan(&request(script));
            assert_eq!(query.probes[0], representative_probe(script));
            for (i, probe) in query.probes.iter().enumerate() {
                assert!(
                    !query.probes[..i].contains(probe),
                    "{script:?} repeats {probe:?}"
                );
            }
        }
    }

    #[test]
    fn latin_request_does_not_probe_latin_twice() {
        assert_eq!(
            plan(&request(Script::Latin)).probes,
            vec![Probe('A'), Probe('★')]
        );
        assert_eq!(
            plan(&request(Script::Han)).probes,
            vec![Probe('一'), Probe('A'), Probe('★')]
        );
    }

    #[test]
    fn bcp47_hint_wins_over_script_default() {
        assert_eq!(locale_name(Script::Han, "ja"), LocaleName("ja".into()));
        assert_eq!(
            locale_name(Script::Han, "zh-Hant"),
            LocaleName("zh-Hant".into())
        );
        assert_eq!(
            locale_name(Script::Latin, "PT_BR"),
            LocaleName("pt-BR".into())
        );
    }

    /// `zh` is what the catalog calls Simplified Chinese, and DirectWrite reads
    /// a bare `zh` as "ask the user's language list" — which answers with a
    /// Japanese face that has no simplified-only glyphs.
    #[test]
    fn a_bare_hint_is_qualified_by_the_script_default() {
        assert_eq!(locale_name(Script::Han, "zh"), LocaleName("zh-Hans".into()));
        assert_eq!(
            locale_name(Script::Han, "zh_hant"),
            LocaleName("zh-Hant".into())
        );
        assert_eq!(
            locale_name(Script::Hiragana, "ja"),
            LocaleName("ja-JP".into())
        );
        assert_eq!(locale_name(Script::Latin, "en"), LocaleName("en-US".into()));
        assert_eq!(locale_name(Script::Latin, "pt"), LocaleName("pt".into()));
    }

    #[test]
    fn tags_are_canonically_cased() {
        assert_eq!(canonical_tag("zh-hant"), "zh-Hant");
        assert_eq!(canonical_tag("PT-br"), "pt-BR");
        assert_eq!(canonical_tag("sr-latn-rs"), "sr-Latn-RS");
    }

    #[test]
    fn absent_or_undetermined_hint_falls_to_script_default() {
        assert_eq!(locale_name(Script::Han, ""), LocaleName("zh-Hans".into()));
        assert_eq!(
            locale_name(Script::Han, "und"),
            LocaleName("zh-Hans".into())
        );
        assert_eq!(
            locale_name(Script::Katakana, "und-Jpan"),
            LocaleName("ja-JP".into())
        );
        assert_eq!(locale_name(Script::Common, ""), LocaleName("en-US".into()));
    }

    #[test]
    fn style_flags_pick_family_weight_and_slant() {
        let plain = plan(&request(Script::Latin));
        assert_eq!(plain.base_family, UI_FAMILY);
        assert_eq!(plain.weight, REGULAR);
        assert_eq!(plain.style, FontStyle::Upright);

        let fancy = plan(&FontRequest {
            bold: true,
            italic: true,
            monospace: true,
            ..request(Script::Latin)
        });
        assert_eq!(fancy.base_family, MONO_FAMILY);
        assert_eq!(fancy.weight, BOLD);
        assert_eq!(fancy.style, FontStyle::Italic);
    }
}
