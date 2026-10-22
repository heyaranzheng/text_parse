use std::fmt;

/// Errors that can occur during text parsing.
#[derive(Debug)]
pub enum ParseError {
    /// No speaker was specified before the first sentence.
    MissingSpeaker,
    /// The emotion name was not found in the dictionary.
    UnknownEmotion(String),
    /// The `(speaker, emotion_code)` pair was not found in the dictionary.
    UnknownTone((String, i32)),
    /// An I/O error occurred while reading the input file.
    Io(std::io::Error),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::MissingSpeaker => write!(
                f,
                "no speaker specified; a speaker must be given at the \
                 head of the sentence like 'name: <text>'"
            ),
            ParseError::UnknownEmotion(name) => {
                write!(f, "unknown emotion name: '{}'", name)
            }
            ParseError::UnknownTone((speaker, code)) => {
                write!(f, "unknown tone for speaker '{}' with emotion code {}", speaker, code)
            }
            ParseError::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParseError::Io(e) => Some(e),
            ParseError::MissingSpeaker
            | ParseError::UnknownEmotion(_)
            | ParseError::UnknownTone(_) => None,
        }
    }
}

impl From<std::io::Error> for ParseError {
    fn from(e: std::io::Error) -> Self {
        ParseError::Io(e)
    }
}