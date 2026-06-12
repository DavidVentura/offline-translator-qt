use qmetaobject::*;
use std::sync::Arc;

use crate::document::DocumentEvent;
use crate::model::{FeatureKind, Language};

use super::{AppBridge, TtsVoiceListItem};

#[derive(Clone)]
pub struct UiCallbacks {
    pub set_languages: Arc<dyn Fn(Vec<Language>) + Send + Sync>,
    pub set_feature_progress: Arc<dyn Fn(String, FeatureKind, f32) + Send + Sync>,
    pub set_input_text: Arc<dyn Fn(String) + Send + Sync>,
    pub set_output_text: Arc<dyn Fn(String) + Send + Sync>,
    pub set_tts_state: Arc<dyn Fn(bool, bool) + Send + Sync>,
    pub set_tts_voices: Arc<dyn Fn(bool, Vec<TtsVoiceListItem>, String, String) + Send + Sync>,
    pub set_processed_image: Arc<dyn Fn(QImage) + Send + Sync>,
    pub notify_live_frame: Arc<dyn Fn() + Send + Sync>,
    pub set_detected_language_code: Arc<dyn Fn(String) + Send + Sync>,
    pub set_document_event: Arc<dyn Fn(DocumentEvent) + Send + Sync>,
}

pub fn create_ui_callbacks(app: QPointer<AppBridge>) -> UiCallbacks {
    let language_app = app.clone();
    let set_languages = queued_callback(move |languages: Vec<Language>| {
        if let Some(app) = language_app.as_pinned() {
            app.borrow_mut().set_languages_value(languages);
        }
    });

    let progress_app = app.clone();
    let set_feature_progress = queued_callback(move |args: (String, i32, f32)| {
        if let Some(app) = progress_app.as_pinned()
            && let Some(feature) = FeatureKind::from_i32(args.1)
        {
            app.borrow_mut()
                .set_feature_progress_value(&args.0, feature, args.2);
        }
    });

    let input_app = app.clone();
    let set_input_text = queued_callback(move |text: String| {
        if let Some(app) = input_app.as_pinned() {
            app.borrow_mut().set_input_text_value(text);
        }
    });

    let output_app = app.clone();
    let set_output_text = queued_callback(move |text: String| {
        if let Some(app) = output_app.as_pinned() {
            app.borrow_mut().set_output_text_value(text);
        }
    });

    let tts_state_app = app.clone();
    let set_tts_state = queued_callback(move |args: (bool, bool)| {
        if let Some(app) = tts_state_app.as_pinned() {
            app.borrow_mut().set_tts_state_value(args.0, args.1);
        }
    });

    let tts_voices_app = app.clone();
    let set_tts_voices =
        queued_callback(move |args: (bool, Vec<TtsVoiceListItem>, String, String)| {
            if let Some(app) = tts_voices_app.as_pinned() {
                app.borrow_mut()
                    .set_tts_voices_value(args.0, args.1, args.2, args.3);
            }
        });

    let processed_image_app = app.clone();
    let set_processed_image = queued_callback(move |image: QImage| {
        if let Some(app) = processed_image_app.as_pinned() {
            app.borrow_mut().set_processed_image_value(image);
        }
    });

    let live_frame_app = app.clone();
    let bump_live_frame_tick = queued_callback(move |_: ()| {
        if let Some(app) = live_frame_app.as_pinned() {
            app.borrow_mut().bump_live_frame_tick();
        }
    });

    let detected_app = app.clone();
    let set_detected_language_code = queued_callback(move |code: String| {
        if let Some(app) = detected_app.as_pinned() {
            app.borrow_mut().set_detected_language_code_value(&code);
        }
    });

    let document_app = app.clone();
    let set_document_event = queued_callback(move |event: DocumentEvent| {
        if let Some(app) = document_app.as_pinned() {
            app.borrow_mut().apply_document_event(event);
        }
    });

    UiCallbacks {
        set_languages: Arc::new(set_languages),
        set_feature_progress: Arc::new(move |code, feature, progress| {
            set_feature_progress((code, feature.as_i32(), progress))
        }),
        set_input_text: Arc::new(set_input_text),
        set_output_text: Arc::new(set_output_text),
        set_tts_state: Arc::new(move |loading, playing| set_tts_state((loading, playing))),
        set_tts_voices: Arc::new(
            move |available, items, selected_name, selected_display_name| {
                set_tts_voices((available, items, selected_name, selected_display_name))
            },
        ),
        set_processed_image: Arc::new(set_processed_image),
        notify_live_frame: Arc::new(move || bump_live_frame_tick(())),
        set_detected_language_code: Arc::new(set_detected_language_code),
        set_document_event: Arc::new(set_document_event),
    }
}
