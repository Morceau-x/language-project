//! Cutting sample programs into tokens, with the reference compiler's
//! lexer, named the way the spec names them.

use maxim_compiler::lexer::lex_tokens;
use maxim_compiler::lexer::token::TokenKind;
use maxim_compiler::span::FileId;

/// The tokens of a sample: their names in the spec (`LOWER`, `"fn"`…)
/// and their texts.
pub struct Tokens {
    pub names: Vec<String>,
    pub texts: Vec<String>,
}

/// Why a sample has no tokens.
pub enum NoTokens {
    /// The reference lexer rejects it.
    LexicalError,
    /// It has HTML in it, which the spec leaves out.
    Html,
}

pub fn lex(text: &str) -> Result<Tokens, NoTokens> {
    let tokens = lex_tokens(FileId(0), text).map_err(|_| NoTokens::LexicalError)?;
    let mut names = Vec::new();
    let mut texts = Vec::new();
    for token in tokens {
        let name = match &token.kind {
            TokenKind::Eof => continue,
            TokenKind::Lower(_) => "LOWER".to_string(),
            TokenKind::Upper(_) => "UPPER".to_string(),
            TokenKind::Int(_) => "INT".to_string(),
            TokenKind::Float(_) => "FLOAT".to_string(),
            TokenKind::Str(_) => "STRING".to_string(),
            TokenKind::StringBegin(_) => "STRING_START".to_string(),
            TokenKind::StringMid(_) => "STRING_MIDDLE".to_string(),
            TokenKind::StringEnd(_) => "STRING_END".to_string(),
            TokenKind::LParenBOL => "BOL_LPAREN".to_string(),
            TokenKind::JsxTagStart(_)
            | TokenKind::JsxClose(_)
            | TokenKind::JsxTagEnd
            | TokenKind::JsxSelfClose
            | TokenKind::JsxAttrName(_)
            | TokenKind::JsxText(_)
            | TokenKind::DollarBrace => return Err(NoTokens::Html),
            // Keywords, punctuation and `_`: their own text, quoted.
            _ => format!("\"{}\"", token.text),
        };
        names.push(name);
        texts.push(token.text);
    }
    Ok(Tokens { names, texts })
}
