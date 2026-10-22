use crate::error::ParseError;

/// Unified dictionary trait for the TTS parsing pipeline.
///
/// Implementations supply two lookups:
///
/// 1. **emotion name → numeric code**  (`dict`)
/// 2. **(speaker, emotion_code) → audio file path**  (`get_tone`)
///
/// # Reference emotion codes
///
/// | Code | Emotion     |
/// |------|-------------|
/// | `0`  | `default`   |
/// | `1`  | `happy`     |
/// | `2`  | `sad`       |
/// | `3`  | `angry`     |
/// | `4`  | `fear`      |
/// | `5`  | `surprise`  |
/// | `6`  | `love`      |
/// | `7`  | `pride`     |
///
/// # Example
///
/// ```rust
/// use text_parse::{Dic, ParseError};
/// use std::collections::HashMap;
///
/// struct MyDict {
///     emotion_map: HashMap<String, i32>,
///     tone_map: HashMap<(String, i32), String>,
/// }
///
/// impl Dic for MyDict {
///     fn dict(&self, name: &str) -> Result<i32, ParseError> {
///         self.emotion_map.get(&name.to_lowercase())
///             .copied()
///             .ok_or(ParseError::UnknownEmotion(name.to_string()))
///     }
///
///     fn get_tone(&self, tag: (String, i32)) -> Result<String, ParseError> {
///         self.tone_map.get(&tag)
///             .cloned()
///             .ok_or(ParseError::UnknownTone(tag.clone()))
///     }
/// }
/// ```
pub trait Dic {
    /// Look up the numeric code for a given emotion name.
    ///
    /// * `name` - The emotion name, e.g. `"happy"` or `"sad"`.
    fn dict(&self, name: &str) -> Result<i32, ParseError>;

    /// Map a `(speaker, emotion_code)` pair to a reference audio file path.
    ///
    /// The returned string is used as `prompt_wav` in generated Python scripts.
    ///
    /// * `tag.0` — speaker name (e.g. `"jarvis"`)
    /// * `tag.1` — emotion code (e.g. `1` for happy)
    fn get_tone(&self, tag: (String, i32)) -> Result<String, ParseError>;
}