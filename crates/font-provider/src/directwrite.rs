//! `FontProvider` backed by DirectWrite.
//!
//! `FontHandle` is a file on disk, so this walks DirectWrite's system fallback
//! all the way down to one: `MapCharacters` picks the family Windows itself
//! would use for a script in a locale, and the resulting `IDWriteFont` is
//! resolved through its font face to the file backing it, its index inside a
//! `.ttc`, and its real weight.
//!
//! Which codepoints get asked about, in which locale, and with which base
//! family is decided in [`crate::dwrite_query`], which has no COM in it. This
//! module is the shell that runs those decisions.

use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::Mutex;

use windows::Win32::Foundation::E_UNEXPECTED;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT, DWRITE_READING_DIRECTION,
    DWRITE_READING_DIRECTION_LEFT_TO_RIGHT, DWriteCreateFactory, IDWriteFactory2, IDWriteFont,
    IDWriteFontCollection, IDWriteFontFallback, IDWriteFontFile, IDWriteLocalFontFileLoader,
    IDWriteNumberSubstitution, IDWriteTextAnalysisSource, IDWriteTextAnalysisSource_Impl,
};
use windows_core::{ComObject, Interface, OutRef, PCWSTR, Result as WinResult, implement};

use translator::font_provider::{FontHandle, FontProvider, FontRequest};
use translator::script::Script;

use crate::dwrite_query::{FallbackQuery, FontStyle, LocaleName, Probe, plan};

pub struct DirectWriteProvider {
    dwrite: Option<SystemFallback>,
    cache: Mutex<HashMap<CacheKey, Vec<FontHandle>>>,
}

/// `MapCharacters` only honours a base family name it can look up, so the
/// collection travels with the fallback object rather than being passed as
/// NULL — with NULL the base family is ignored outright and every request
/// answers with whatever the system fallback tables hold, monospace included.
struct SystemFallback {
    fallback: IDWriteFontFallback,
    collection: IDWriteFontCollection,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    script: Script,
    language: String,
    bold: bool,
    italic: bool,
    monospace: bool,
}

impl DirectWriteProvider {
    pub fn new() -> Self {
        let dwrite = match system_fallback() {
            Ok(dwrite) => Some(dwrite),
            Err(e) => {
                eprintln!("DirectWrite init failed ({e}); overlay text will fall back to tofu");
                None
            }
        };
        Self {
            dwrite,
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn locate_uncached(&self, request: &FontRequest) -> Vec<FontHandle> {
        let Some(dwrite) = self.dwrite.as_ref() else {
            return Vec::new();
        };

        let query = plan(request);
        let mut chain: Vec<FontHandle> = Vec::with_capacity(query.probes.len());
        for probe in &query.probes {
            match resolve(dwrite, &query, *probe) {
                Ok(Some(handle)) => {
                    if !chain.contains(&handle) {
                        chain.push(handle);
                    }
                }
                Ok(None) => {}
                Err(e) => eprintln!(
                    "DirectWrite fallback for U+{:04X} failed: {e}",
                    probe.0 as u32
                ),
            }
        }
        chain
    }
}

impl FontProvider for DirectWriteProvider {
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

/// The shared factory is a process-wide singleton and the objects it hands out
/// keep it alive, so nothing else needs holding on to.
fn system_fallback() -> WinResult<SystemFallback> {
    let factory: IDWriteFactory2 = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
    let fallback = unsafe { factory.GetSystemFontFallback()? };
    let mut collection: Option<IDWriteFontCollection> = None;
    unsafe { factory.GetSystemFontCollection(&mut collection, false)? };
    let collection = collection.ok_or_else(|| {
        windows_core::Error::new(
            E_UNEXPECTED,
            "GetSystemFontCollection returned no collection",
        )
    })?;
    Ok(SystemFallback {
        fallback,
        collection,
    })
}

/// `Ok(None)` when DirectWrite maps the probe to nothing, or to a face that has
/// no file on disk to point a `FontHandle` at.
fn resolve(
    dwrite: &SystemFallback,
    query: &FallbackQuery,
    probe: Probe,
) -> WinResult<Option<FontHandle>> {
    let Some(font) = map_characters(dwrite, query, probe)? else {
        return Ok(None);
    };

    let face = unsafe { font.CreateFontFace()? };
    let mut file_count = 0u32;
    unsafe { face.GetFiles(&mut file_count, None)? };
    if file_count != 1 {
        // A face assembled from several files (a raw CFF plus its metrics, say)
        // is not something a single path can name.
        return Ok(None);
    }

    let mut files: [Option<IDWriteFontFile>; 1] = [None];
    unsafe { face.GetFiles(&mut file_count, Some(files.as_mut_ptr()))? };
    let file = files[0]
        .as_ref()
        .expect("GetFiles counted one file and then wrote none");
    let Some(path) = local_path(file)? else {
        return Ok(None);
    };

    let weight = unsafe { font.GetWeight() }.0;
    let weight = u16::try_from(weight).expect("DirectWrite reported a weight outside 1..=999");
    Ok(Some(
        FontHandle::new(path, unsafe { face.GetIndex() }).with_weight(weight),
    ))
}

fn map_characters(
    dwrite: &SystemFallback,
    query: &FallbackQuery,
    probe: Probe,
) -> WinResult<Option<IDWriteFont>> {
    let source: IDWriteTextAnalysisSource =
        ComObject::new(ProbeSource::new(probe, &query.locale)).into_interface();
    let base_family = wide(query.base_family.0);
    let style = match query.style {
        FontStyle::Upright => DWRITE_FONT_STYLE_NORMAL,
        FontStyle::Italic => DWRITE_FONT_STYLE_ITALIC,
    };

    let mut mapped_length = 0u32;
    let mut mapped_font: Option<IDWriteFont> = None;
    let mut scale = 0f32;
    unsafe {
        dwrite.fallback.MapCharacters(
            &source,
            0,
            probe.0.len_utf16() as u32,
            &dwrite.collection,
            PCWSTR(base_family.as_ptr()),
            DWRITE_FONT_WEIGHT(i32::from(query.weight.0)),
            style,
            DWRITE_FONT_STRETCH_NORMAL,
            &mut mapped_length,
            &mut mapped_font,
            &mut scale,
        )?
    };
    Ok(mapped_font)
}

/// `Ok(None)` for a face whose bytes come from a custom loader — a memory or
/// network font has no path, and inventing one would be worse than skipping it.
fn local_path(file: &IDWriteFontFile) -> WinResult<Option<PathBuf>> {
    let mut key = std::ptr::null_mut();
    let mut key_size = 0u32;
    unsafe { file.GetReferenceKey(&mut key, &mut key_size)? };

    let loader = unsafe { file.GetLoader()? };
    let Ok(local) = loader.cast::<IDWriteLocalFontFileLoader>() else {
        return Ok(None);
    };

    let length = unsafe { local.GetFilePathLengthFromKey(key, key_size)? };
    let mut path = vec![0u16; length as usize + 1];
    unsafe { local.GetFilePathFromKey(key, key_size, &mut path)? };
    path.truncate(length as usize);
    Ok(Some(PathBuf::from(OsString::from_wide(&path))))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The single-codepoint text `MapCharacters` analyses. DirectWrite reads the
/// text and the locale back out through this interface rather than taking them
/// as arguments, so a probe has to be handed over as a one-character document.
#[implement(IDWriteTextAnalysisSource)]
struct ProbeSource {
    text: Vec<u16>,
    locale: Vec<u16>,
}

impl ProbeSource {
    fn new(probe: Probe, locale: &LocaleName) -> Self {
        let mut text = [0u16; 2];
        let text = probe.0.encode_utf16(&mut text).to_vec();
        Self {
            text,
            locale: wide(&locale.0),
        }
    }
}

impl IDWriteTextAnalysisSource_Impl for ProbeSource_Impl {
    fn GetTextAtPosition(
        &self,
        textposition: u32,
        textstring: *mut *mut u16,
        textlength: *mut u32,
    ) -> WinResult<()> {
        let start = textposition as usize;
        if start >= self.text.len() {
            unsafe {
                *textstring = std::ptr::null_mut();
                *textlength = 0;
            }
            return Ok(());
        }
        unsafe {
            *textstring = self.text.as_ptr().add(start).cast_mut();
            *textlength = (self.text.len() - start) as u32;
        }
        Ok(())
    }

    fn GetTextBeforePosition(
        &self,
        textposition: u32,
        textstring: *mut *mut u16,
        textlength: *mut u32,
    ) -> WinResult<()> {
        let end = textposition as usize;
        if end == 0 || end > self.text.len() {
            unsafe {
                *textstring = std::ptr::null_mut();
                *textlength = 0;
            }
            return Ok(());
        }
        unsafe {
            *textstring = self.text.as_ptr().cast_mut();
            *textlength = end as u32;
        }
        Ok(())
    }

    /// Direction steers line layout, not which family covers a codepoint, and
    /// the probe is one character long either way.
    fn GetParagraphReadingDirection(&self) -> DWRITE_READING_DIRECTION {
        DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
    }

    fn GetLocaleName(
        &self,
        textposition: u32,
        textlength: *mut u32,
        localename: *mut *mut u16,
    ) -> WinResult<()> {
        let start = textposition as usize;
        unsafe {
            *textlength = self.text.len().saturating_sub(start) as u32;
            *localename = self.locale.as_ptr().cast_mut();
        }
        Ok(())
    }

    fn GetNumberSubstitution(
        &self,
        textposition: u32,
        textlength: *mut u32,
        numbersubstitution: OutRef<'_, IDWriteNumberSubstitution>,
    ) -> WinResult<()> {
        let start = textposition as usize;
        unsafe { *textlength = self.text.len().saturating_sub(start) as u32 };
        numbersubstitution.write(None)
    }
}
