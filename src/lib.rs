mod dic;
mod error;
mod text_parse;

pub use dic::Dic;
pub use text_parse::Parser;
pub use text_parse::generate_python_scripts;
pub use error::ParseError;