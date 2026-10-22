/// # A TTS (Text-to-Speech) tool
///
/// This script splits a text file into multiple sentences first,
/// then attaches an emotion tag to each part of the sentence if the sentence contains
/// some emotion words. Finally, add the emotion sentences to a vector.
///
/// ## Notes
///
/// 1. The emotion tags are marked in brackets.
///    - For example: `[happy]` or `[sad]`.
///
/// 2. The emotion tags should be at the beginning of the sentence, or the beginning of the
///    part. If the sentence contains multiple emotion words, the last emotion word will cover
///    the previous emotion words when the part is split.
///    For example:
///    - `[happy] I am happy today, happy.`
///      -> part1: `[happy] + I am happy today`
///      -> part2: `[happy] + happy`
///    - `[happy] I find a job today, [sad] but it is not good.`
///      -> part1: `[happy] + I find a job today`
///      -> part2: `[sad] + but it is not good`
///    - `[happy] I am happy today.  I am sad today`
///      -> part1: `[happy] + I am happy today`
///      -> part2: `[happy] + I am sad today`
///
/// 3. If we have no emotion words in the sentence, the sentence will not be split.
///    And the emotion will be treated as a default emotion.
///
/// ## The Emotion Constructor
///
/// `[happy]`, `[sad]`, ... or something like that is not a real emotion tag.
/// An emotion tag is a bracketed pair of numbers and names.
/// It's a tuple `(speaker, emotion)`:
/// 1. The first place is the name of the speaker, or the speaker's tone. It's a string.
/// 2. The second place is the emotion. It's a number.
///    - `0`: the default emotion
///    - `1`: happy
///    - `2`: sad
///    - `3`: angry
///    - `4`: fear
///    - `5`: surprise
///    - `6`: love
///    - `7`: pride
///
/// The Speaker must be specified at the head of the sentence. Use `:` to separate the
/// speaker and the sentence.
///
/// For example:
/// `jarvis: [happy] I am happy today.`
/// -> `(jarvis, 1, ['I am happy today'])`
///
/// If the next sentence has no speaker, the speaker will be treated as the previous speaker.
/// For example:
/// `jarvis: I am happy today. [sad] But i am not good yesterday. father: [happy] I am happy today.`
/// -> `(jarvis, 1, ['I am happy today'])`
/// -> `(jarvis, 2, ['But i am not good yesterday'])`
/// -> `(father, 1, ['I am happy today'])`
///
/// ## Output
///
/// A list of tuples: `(speaker, emotion, sentence_parts)`
///
/// Every sentence will be converted to a tuple like that.

use super::Dic;
use crate::error::ParseError;
use regex::Regex;
use std::fs;

/// Split the raw text into tokens, keeping every character.
///
/// Recognises special tokens:
/// - A **speaker** (`"name:"`), e.g. `jarvis:`
/// - An **emotion tag** (`"[name]"`), e.g. `[happy]`
/// - A **strong boundary** (`"."`, `"!"`, `"?"`) — ends a sentence and a unit
/// - A **weak boundary** (`","`, `";"`) — ends a part but keeps the unit alive
///
/// Names support Chinese, Latin letters, and underscores via `\p{Han}`.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut pos = 0;
    let re = Regex::new(
        r"[\p{Han}A-Za-z_]+:|\[[\p{Han}A-Za-z_]+\]|[.!?]|[,;]",
    )
    .expect("invalid TOKEN_PATTERN");
    for m in re.find_iter(text) {
        if m.start() > pos {
            tokens.push(text[pos..m.start()].to_string());
        }
        tokens.push(m.as_str().to_string());
        pos = m.end();
    }
    if pos < text.len() {
        tokens.push(text[pos..].to_string());
    }
    tokens
}

/// A push-down state machine that mirrors the Python version's parsing logic.
///
/// It tracks:
/// - The current **speaker** (propagated across sentences when missing)
/// - An **explicit emotion** set by `[tag]` tokens (last one wins)
/// - A **text emotion** inferred from emotion words found in ordinary text
/// - **Parts**: completed sub-sentence segments (split on weak boundaries)
/// - **cur_part**: text currently being accumulated for the current part
/// - **result**: the final list of `(speaker, emotion_code, sentence_parts)` tuples
pub struct Parser<'a, D: Dic> {
    dict: &'a D,
    speaker: Option<String>,
    explicit_emotion: Option<String>,
    text_emotion: Option<String>,
    parts: Vec<String>,
    cur_part: String,
    result: Vec<(String, i32, Vec<String>)>,
}

impl<'a, D: Dic> Parser<'a, D> {
    pub fn new(dict: &'a D) -> Self {
        Parser {
            dict,
            speaker: None,
            explicit_emotion: None,
            text_emotion: None,
            parts: Vec::new(),
            cur_part: String::new(),
            result: Vec::new(),
        }
    }

    /// Compute the effective emotion: explicit tag wins over text-emotion,
    /// otherwise fall back to `"default"`.
    fn compute_emotion(&self) -> Result<i32, ParseError> {
        let name = self
            .explicit_emotion
            .as_deref()
            .or(self.text_emotion.as_deref())
            .unwrap_or("default");
        self.dict.dict(name)
    }

    /// Close the current part: trim surrounding whitespace, and if non-empty,
    /// push it onto the `parts` list.
    fn close_part(&mut self) {
        let t = self.cur_part.trim().to_string();
        if !t.is_empty() {
            self.parts.push(t);
        }
        self.cur_part.clear();
    }

    /// Close the whole sentence unit: emit one `(speaker, emotion, parts)` tuple.
    ///
    /// Returns `Err(ParseError::MissingSpeaker)` if no speaker has been specified yet.
    ///
    /// If the sentence was split into multiple parts, strip the separators
    /// (punctuation) that were kept at part boundaries.
    fn close_unit(&mut self) -> Result<(), ParseError> {
        self.close_part();
        if self.parts.is_empty() {
            return Ok(());
        }

        let spk = match &self.speaker {
            Some(s) => s.clone(),
            None => return Err(ParseError::MissingSpeaker),
        };

        // If the sentence was split into multiple parts, drop the
        // separators (punctuation) kept at the part boundaries.
        if self.parts.len() > 1 {
            let stripset = |c: char| matches!(c, '.' | ',' | ';' | ':' | '!' | '?');
            for p in self.parts.iter_mut() {
                let trimmed = p.trim_end_matches(stripset).to_string();
                *p = trimmed;
            }
        }

        let emotion = self.compute_emotion()?;
        let parts = std::mem::take(&mut self.parts);
        self.result.push((spk, emotion, parts));

        self.explicit_emotion = None;
        self.text_emotion = None;

        Ok(())
    }

    /// Scan a chunk of ordinary text for emotion words.
    ///
    /// An emotion word in the text (e.g. `"happy"`). We scan the text to infer the
    /// emotion when there is no explicit emotion tag. The **last** emotion word
    /// found covers any previous ones in the same unit.
    fn scan_emotion(&mut self, text: &str) {
        let re = Regex::new(r"(?i)\b(happy|sad|angry|fear|surprise|love|pride)\b")
            .expect("invalid EMOTION_WORD_PATTERN");
        for m in re.find_iter(text) {
            self.text_emotion = Some(m.as_str().to_string().to_lowercase());
        }
    }

    /// Parse the raw text into `(speaker, emotion_code, sentence_parts)` tuples.
    ///
    /// The speaker is introduced at the head of a sentence with `":"` and is propagated
    /// to the following sentences when they have no speaker of their own.
    ///
    /// Every sentence becomes one tuple; the emotion code is the explicit tag if present
    /// (the last one wins), otherwise the last emotion word found in the sentence,
    /// otherwise the default emotion. The mapping from name to code is delegated to
    /// the [`Dic`] held by this parser.
    pub fn parse_text(&mut self, text: &str) -> Result<Vec<(String, i32, Vec<String>)>, ParseError> {
        self.reset();

        let speaker_re = Regex::new(r"^[\p{Han}A-Za-z_]+:$").expect("invalid speaker regex");
        let tag_re = Regex::new(r"^\[([\p{Han}A-Za-z_]+)\]$").expect("invalid tag regex");

        for token in tokenize(text) {
            if token.is_empty() {
                continue;
            }

            // Speaker marker: "name:"
            if speaker_re.is_match(&token) {
                self.close_unit()?;
                self.speaker = Some(token[..token.len() - 1].to_string());
                continue;
            }

            // Emotion tag: "[name]"
            if let Some(caps) = tag_re.captures(&token) {
                self.close_unit()?;
                self.explicit_emotion =
                    Some(caps.get(1).unwrap().as_str().to_string().to_lowercase());
                continue;
            }

            // Strong boundary: end of sentence and end of unit.
            if matches!(token.as_str(), "." | "!" | "?") {
                self.cur_part.push_str(&token);
                self.close_part();
                self.close_unit()?;
                continue;
            }

            // Weak boundary: it ends the current part but keeps the unit alive.
            if matches!(token.as_str(), "," | ";") {
                self.cur_part.push_str(&token);
                self.close_part();
                continue;
            }

            // Ordinary text: accumulate it and scan for emotion words.
            self.scan_emotion(&token);
            self.cur_part.push_str(&token);
        }

        self.close_unit()?;
        Ok(std::mem::take(&mut self.result))
    }

    /// Read a text file and reshape it into `(speaker, emotion_code, sentence_parts)` tuples.
    pub fn parse_file(&mut self, filepath: &str) -> Result<Vec<(String, i32, Vec<String>)>, ParseError> {
        let content = fs::read_to_string(filepath)?;
        self.parse_text(&content)
    }

    /// Parse text → generate Python TTS scripts in one call.
    ///
    /// Internally calls [`Self::parse_text`] then [`generate_python_scripts`].
    /// The `tone_dic` (same `Dic` instance) maps each `(speaker, emotion_code)`
    /// pair to a reference audio file used as `prompt_wav` in the generated script.
    pub fn parse_to_python(
        &mut self,
        text: &str,
        tone_dic: &D,
        config_path: &str,
        device: &str,
        lang: &str,
        verbose: bool,
    ) -> Result<Vec<String>, ParseError> {
        let parsed = self.parse_text(text)?;
        generate_python_scripts(&parsed, tone_dic, config_path, device, lang, verbose)
    }

    /// Reset the parser's internal state so it is ready for a fresh parse.
    fn reset(&mut self) {
        self.speaker = None;
        self.explicit_emotion = None;
        self.text_emotion = None;
        self.parts.clear();
        self.cur_part.clear();
        self.result.clear();
    }
}

/// Generate Python TTS scripts from parsed `(speaker, emotion, parts)` tuples.
///
/// Each sentence **part** yields one complete Python script (following the
/// ConfuciusTTS inference template).  The scripts are numbered sequentially
/// (`output_000001.wav`, `output_000002.wav`, …) so that the caller can write
/// them to identically‑named `.py` files and execute with a shell loop.
///
/// * `parsed`  – the output of [`Parser::parse_text`] or [`Parser::parse_file`]
/// * `tone_dic` – maps `(speaker, emotion_code)` to a reference audio file
///   (used as `prompt_wav`)
/// * `config_path` – path to the TTS inference config YAML
/// * `device` – `"cpu"` or `"cuda"`
/// * `lang` – language code, e.g. `"zh"`
/// * `verbose` – passed through to `model.generate(verbose=…)`
pub fn generate_python_scripts<T: Dic>(
    parsed: &[(String, i32, Vec<String>)],
    tone_dic: &T,
    config_path: &str,
    device: &str,
    lang: &str,
    verbose: bool,
) -> Result<Vec<String>, ParseError> {
    let mut scripts = Vec::new();
    let mut index: usize = 0;

    for (speaker, emotion_code, parts) in parsed {
        let prompt_wav = tone_dic.get_tone((speaker.clone(), *emotion_code))?;
        for part in parts {
            index += 1;
            let escaped_text = part.replace('\\', "\\\\").replace('"', "\\\"");
            let script = format!(
                r#"import torch
import torchaudio
from confuciustts.cli.inference import ConfuciusTTS

model = ConfuciusTTS(
    config_path="{config_path}",
    device="{device}",
)

audio = model.generate(
    text="{text}",
    lang="{lang}",
    prompt_wav="{prompt_wav}",
    verbose={verbose},
)

torchaudio.save("output_{index:06}.wav", audio.cpu(), model.sample_rate)
print(f"音频已保存至 output_{index:06}.wav，采样率为 {{model.sample_rate}} Hz")"#,
                config_path = config_path,
                device = device,
                text = escaped_text,
                lang = lang,
                prompt_wav = prompt_wav,
                verbose = if verbose { "True" } else { "False" },
                index = index,
            );
            scripts.push(script);
        }
    }

    Ok(scripts)
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct TestDict {
        map: HashMap<String, i32>,
    }

    impl TestDict {
        fn new() -> Self {
            let mut map = HashMap::new();
            map.insert("default".to_string(), 0);
            map.insert("happy".to_string(), 1);
            map.insert("sad".to_string(), 2);
            map.insert("angry".to_string(), 3);
            map.insert("fear".to_string(), 4);
            map.insert("surprise".to_string(), 5);
            map.insert("love".to_string(), 6);
            map.insert("pride".to_string(), 7);
            TestDict { map }
        }
    }

    impl Dic for TestDict {
        fn dict(&self, name: &str) -> Result<i32, ParseError> {
            self.map
                .get(&name.to_lowercase())
                .copied()
                .ok_or(ParseError::UnknownEmotion(name.to_string()))
        }

        fn get_tone(&self, tag: (String, i32)) -> Result<String, ParseError> {
            Ok(format!("/home/aran/Music/{}_{}.wav", tag.0, tag.1))
        }
    }

    fn test_dict() -> TestDict {
        TestDict::new()
    }

    #[test]
    fn speaker_propagation() {
        let dict = test_dict();
        let mut p = Parser::new(&dict);
        let out = p
            .parse_text(
                "jarvis: I am happy today. [sad] But i am not good yesterday. \
                 father: [happy] I am happy today.",
            )
            .unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0], ("jarvis".to_string(), 1, vec!["I am happy today.".into()]));
        assert_eq!(
            out[1],
            ("jarvis".to_string(), 2, vec!["But i am not good yesterday.".into()])
        );
        assert_eq!(out[2], ("father".to_string(), 1, vec!["I am happy today.".into()]));
    }

    #[test]
    fn chinese_speaker() {
        let dict = test_dict();
        let mut p = Parser::new(&dict);
        let out = p.parse_text("小明: 我今天很开心。").unwrap();
        assert_eq!(out[0].0, "小明".to_string());
    }

    #[test]
    fn split_drops_punctuation() {
        let dict = test_dict();
        let mut p = Parser::new(&dict);
        let out = p.parse_text("jarvis: I am happy today, happy.").unwrap();
        assert_eq!(out[0].2, vec!["I am happy today".to_string(), "happy".to_string()]);
    }

    #[test]
    fn unsplit_keeps_period() {
        let dict = test_dict();
        let mut p = Parser::new(&dict);
        let out = p.parse_text("jarvis: I am happy today.").unwrap();
        assert_eq!(out[0].2, vec!["I am happy today.".to_string()]);
    }

    #[test]
    fn missing_speaker_returns_error() {
        let dict = test_dict();
        let mut p = Parser::new(&dict);
        let result = p.parse_text("I am happy today.");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ParseError::MissingSpeaker));
    }
}