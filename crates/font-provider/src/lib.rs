//! System font discovery for `translator`'s PDF writer and image renderers.
//!
//! Which platform API backs it is this crate's business, not the caller's:
//! there is a single entry point, [`system_fonts`], and the implementations
//! behind it live one per module.

use std::sync::{Arc, OnceLock};

use translator::font_provider::FontProvider;

#[cfg(unix)]
mod fontconfig;

pub type SharedFontProvider = Arc<dyn FontProvider + Send + Sync>;

/// The process-wide provider. Shared because the backing implementations hold
/// a query cache that is worth keeping warm across pages and camera frames.
pub fn system_fonts() -> SharedFontProvider {
    static INSTANCE: OnceLock<SharedFontProvider> = OnceLock::new();
    INSTANCE.get_or_init(build_provider).clone()
}

#[cfg(unix)]
fn build_provider() -> SharedFontProvider {
    Arc::new(fontconfig::FontconfigProvider::new())
}

/// Null font provider for platforms without fontconfig. Overlay and PDF text
/// render as tofu until a native provider (e.g. DirectWrite) is wired up here.
#[cfg(not(unix))]
fn build_provider() -> SharedFontProvider {
    Arc::new(translator::font_provider::NoFontProvider)
}
