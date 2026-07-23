use qmetaobject::*;

use crate::catalog_state::{format_size, total_size};
use crate::model::Language;

#[derive(Clone, Default, SimpleListItem)]
pub struct LanguageListItem {
    pub code: QString,
    pub name: QString,
    pub size: QString,
    pub installed: bool,
    pub download_progress: f32,
    pub built_in: bool,
}

#[derive(Clone, Default, SimpleListItem)]
pub struct ManageLanguageListItem {
    pub code: QString,
    pub name: QString,
    pub total_size: QString,
    pub built_in: bool,
    pub expanded: bool,
    pub core_available: bool,
    pub core_installed: bool,
    pub core_size: QString,
    pub core_progress: f32,
    pub dictionary_available: bool,
    pub dictionary_installed: bool,
    pub dictionary_size: QString,
    pub dictionary_progress: f32,
    pub tts_available: bool,
    pub tts_installed: bool,
    pub tts_size: QString,
    pub tts_progress: f32,
}

#[derive(Clone, Default, SimpleListItem)]
pub struct TtsVoiceListItem {
    pub name: QString,
    pub display_name: QString,
}

/// One merged selection highlight, in image-pixel space. QML scales these by the same factor it
/// scales the image itself.
#[derive(Clone, Default, SimpleListItem)]
pub struct SelectionPillItem {
    pub cx: f32,
    pub cy: f32,
    pub width: f32,
    pub height: f32,
    pub angle_degrees: f32,
}

#[derive(Clone, Default, SimpleListItem)]
pub struct DictionaryPopupRowItem {
    pub kind: QString,
    pub text: QString,
}

#[derive(Clone, Default, SimpleListItem)]
pub struct ManageTtsVoicePackListItem {
    pub pack_id: QString,
    pub region_display_name: QString,
    pub voice_display_name: QString,
    pub quality_text: QString,
    pub size_text: QString,
    pub installed: bool,
}

pub(crate) fn language_to_list_item(language: Language) -> LanguageListItem {
    LanguageListItem {
        code: QString::from(language.code.clone()),
        name: QString::from(language.name),
        size: QString::from(format_size(language.core_size_bytes)),
        installed: language.core_installed,
        download_progress: language.core_progress,
        built_in: language.built_in,
    }
}

pub(crate) fn manage_language_to_list_item(
    language: &Language,
    expanded: bool,
) -> ManageLanguageListItem {
    ManageLanguageListItem {
        code: QString::from(language.code.clone()),
        name: QString::from(language.name.clone()),
        total_size: QString::from(format_size(total_size(language))),
        built_in: language.built_in,
        expanded,
        core_available: language.core_size_bytes > 0,
        core_installed: language.core_installed,
        core_size: QString::from(format_size(language.core_size_bytes)),
        core_progress: language.core_progress,
        dictionary_available: language.dictionary_size_bytes > 0,
        dictionary_installed: language.dictionary_installed,
        dictionary_size: QString::from(format_size(language.dictionary_size_bytes)),
        dictionary_progress: language.dictionary_progress,
        tts_available: language.tts_size_bytes > 0,
        tts_installed: language.tts_installed,
        tts_size: QString::from(format_size(language.tts_size_bytes)),
        tts_progress: language.tts_progress,
    }
}

pub(crate) fn update_progress_list_item(
    model: &mut SimpleListModel<LanguageListItem>,
    language: &Language,
    available_list: bool,
) {
    let target_code = QString::from(language.code.clone());
    let index = { model.iter().position(|item| item.code == target_code) };
    if let Some(index) = index {
        let should_be_visible = if available_list {
            !language.core_installed && !language.built_in
        } else {
            language.core_installed || language.built_in
        };
        if should_be_visible {
            model.change_line(index, language_to_list_item(language.clone()));
        }
    }
}

pub(crate) fn update_manage_progress_item(
    model: &mut SimpleListModel<ManageLanguageListItem>,
    language: &Language,
    expanded: bool,
) {
    let target_code = QString::from(language.code.clone());
    let index = { model.iter().position(|item| item.code == target_code) };
    if let Some(index) = index {
        model.change_line(index, manage_language_to_list_item(language, expanded));
    }
}
