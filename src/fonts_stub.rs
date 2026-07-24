//! Null font provider for platforms without fontconfig. Overlay and PDF text
//! render as tofu until a native provider (e.g. DirectWrite) is wired up here.

use std::sync::{Arc, OnceLock};

use translator::font_provider::{FontHandle, FontProvider, FontRequest};

pub fn provider() -> Arc<NullFontProvider> {
    static INSTANCE: OnceLock<Arc<NullFontProvider>> = OnceLock::new();
    INSTANCE.get_or_init(|| Arc::new(NullFontProvider)).clone()
}

pub struct NullFontProvider;

impl FontProvider for NullFontProvider {
    fn locate(&self, _request: &FontRequest) -> Vec<FontHandle> {
        Vec::new()
    }
}
