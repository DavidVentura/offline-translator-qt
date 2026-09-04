pub use translator::document::{DocumentError, DocumentFormat, DocumentProgress};

/// What the translation job reports back to the UI thread.
#[derive(Debug, Clone)]
pub enum DocumentEvent {
    Progress(DocumentProgress),
    Done { output_path: String },
    Failed { message: String },
    Cancelled,
}
