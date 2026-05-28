//! `FontProvider` backed by libfontconfig.
//!
//! Used by both the live camera path and the still-image overlay renderer in
//! `translator::image_render`. Maps a `FontRequest` to an `fc-match`-style
//! pattern, sorts the system catalogue with `FcFontSort`, and returns the
//! resulting ranked chain as `FontHandle`s (path + ttc index). `image_render`
//! walks that chain per codepoint, so the chain doubles as the script-fallback
//! list.

use std::ffi::CString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use fontconfig::{
    self, FC_FAMILY, FC_LANG, FC_SLANT, FC_SLANT_ITALIC, FC_SPACING, FC_WEIGHT, FC_WEIGHT_BOLD,
    Fontconfig, Pattern, UnicodeCoverage,
};
use std::collections::HashMap;

use translator::font_provider::{FontHandle, FontProvider, FontRequest};
use translator::script::Script;

const MAX_CHAIN: usize = 8;
const FC_MONO: i32 = 100;

pub fn provider() -> Arc<FontconfigProvider> {
    static INSTANCE: OnceLock<Arc<FontconfigProvider>> = OnceLock::new();
    INSTANCE
        .get_or_init(|| Arc::new(FontconfigProvider::new()))
        .clone()
}

pub struct FontconfigProvider {
    fc: Option<Fontconfig>,
    cache: Mutex<HashMap<CacheKey, Vec<FontHandle>>>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    script: Script,
    language: String,
    bold: bool,
    italic: bool,
    monospace: bool,
}

impl FontconfigProvider {
    fn new() -> Self {
        let fc = Fontconfig::new();
        if fc.is_none() {
            eprintln!("fontconfig init failed; overlay text will fall back to tofu");
        }
        Self {
            fc,
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn locate_uncached(&self, request: &FontRequest) -> Vec<FontHandle> {
        let Some(fc) = self.fc.as_ref() else {
            return Vec::new();
        };

        let Ok(mut pat) = Pattern::new(fc) else {
            return Vec::new();
        };

        let lang_tag = lang_tag_for(request.script, &request.language);
        if let Ok(c) = CString::new(lang_tag) {
            let _ = pat.add_string(FC_LANG, &c);
        }

        if request.monospace {
            let _ = pat.add_integer(FC_SPACING, FC_MONO);
        } else {
            let family = generic_family_for(request.script);
            if let Ok(c) = CString::new(family) {
                let _ = pat.add_string(FC_FAMILY, &c);
            }
        }

        if request.bold {
            let _ = pat.add_integer(FC_WEIGHT, FC_WEIGHT_BOLD);
        }
        if request.italic {
            let _ = pat.add_integer(FC_SLANT, FC_SLANT_ITALIC);
        }

        let Ok(set) = pat.sort_fonts(UnicodeCoverage::Trim) else {
            return Vec::new();
        };

        let mut out = Vec::new();
        for p in set.iter() {
            let Ok(file) = p.filename() else { continue };
            let index = p.face_index().unwrap_or(0).max(0) as u32;
            out.push(FontHandle::new(PathBuf::from(file), index));
            if out.len() >= MAX_CHAIN {
                break;
            }
        }
        out
    }
}

impl FontProvider for FontconfigProvider {
    fn locate(&self, request: &FontRequest) -> Vec<FontHandle> {
        let key = CacheKey {
            script: request.script,
            language: request.language.clone(),
            bold: request.bold,
            italic: request.italic,
            monospace: request.monospace,
        };
        if let Some(hit) = self.cache.lock().unwrap().get(&key) {
            return hit.clone();
        }
        let chain = self.locate_uncached(request);
        self.cache.lock().unwrap().insert(key, chain.clone());
        chain
    }
}

/// Generic CSS-style family that fontconfig knows how to alias to a real font
/// in the user's catalogue (`sans-serif` → DejaVu Sans / Noto Sans / etc.).
fn generic_family_for(script: Script) -> &'static str {
    match script {
        Script::Han | Script::Hiragana | Script::Katakana | Script::Hangul => "sans-serif",
        _ => "sans-serif",
    }
}

/// Map our internal script + BCP-47 hint to a fontconfig lang tag. Prefer the
/// caller's BCP-47 when present (it disambiguates Han variants — `zh-cn` vs
/// `ja` vs `ko` resolve to different .ttc indices); otherwise fall back to a
/// script-default.
fn lang_tag_for(script: Script, language: &str) -> String {
    let primary = language
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_lowercase();
    if !primary.is_empty() && primary != "und" {
        return primary;
    }
    match script {
        Script::Latin => "en",
        Script::Cyrillic => "ru",
        Script::Greek => "el",
        Script::Armenian => "hy",
        Script::Hebrew => "he",
        Script::Arabic => "ar",
        Script::Devanagari => "hi",
        Script::Bengali => "bn",
        Script::Gurmukhi => "pa",
        Script::Gujarati => "gu",
        Script::Oriya => "or",
        Script::Tamil => "ta",
        Script::Telugu => "te",
        Script::Kannada => "kn",
        Script::Malayalam => "ml",
        Script::Sinhala => "si",
        Script::Thai => "th",
        Script::Lao => "lo",
        Script::Tibetan => "bo",
        Script::Myanmar => "my",
        Script::Georgian => "ka",
        Script::Ethiopic => "am",
        Script::Khmer => "km",
        Script::Han => "zh-cn",
        Script::Hiragana | Script::Katakana => "ja",
        Script::Hangul => "ko",
        Script::Common | Script::Inherited | Script::Other => "en",
    }
    .to_string()
}
