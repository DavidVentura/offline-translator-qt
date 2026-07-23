#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    FromOnly,
    ToOnly,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureKind {
    Core,
    Dictionary,
    Tts,
}

impl From<FeatureKind> for translator::Feature {
    fn from(value: FeatureKind) -> Self {
        match value {
            FeatureKind::Core => translator::Feature::Core,
            FeatureKind::Dictionary => translator::Feature::Dictionary,
            FeatureKind::Tts => translator::Feature::Tts,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TtsVoicePackOption {
    pub pack_id: String,
    pub display_name: String,
    pub quality: Option<String>,
    pub size_bytes: u64,
    pub installed: bool,
}

#[derive(Clone, Debug, Default)]
pub struct TtsVoicePickerRegion {
    pub code: String,
    pub display_name: String,
    pub voices: Vec<TtsVoicePackOption>,
}

#[derive(Clone, Debug, Default)]
pub struct Language {
    pub code: String,
    pub name: String,
    pub dictionary_code: String,
    pub direction: Direction,
    pub built_in: bool,
    pub core_size_bytes: u64,
    pub core_installed: bool,
    pub core_progress: f32,
    pub dictionary_size_bytes: u64,
    pub dictionary_installed: bool,
    pub dictionary_progress: f32,
    pub tts_size_bytes: u64,
    pub tts_installed: bool,
    pub tts_progress: f32,
    pub tts_voice_picker_regions: Vec<TtsVoicePickerRegion>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    NoLanguages = 0,
    Translation = 1,
    Settings = 2,
    ManageLanguages = 3,
}

impl Screen {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

impl Default for Direction {
    fn default() -> Self {
        Self::Both
    }
}

impl FeatureKind {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Core),
            1 => Some(Self::Dictionary),
            2 => Some(Self::Tts),
            _ => None,
        }
    }

    pub fn as_i32(self) -> i32 {
        match self {
            Self::Core => 0,
            Self::Dictionary => 1,
            Self::Tts => 2,
        }
    }
}

impl Default for Screen {
    fn default() -> Self {
        Self::NoLanguages
    }
}
