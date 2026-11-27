// SPDX-License-Identifier: MIT
//! Source text to tokens.

use crate::ScriptError;

/// Largest source accepted, in bytes.
pub const MAX_SOURCE: usize = 1 << 20;
/// Longest string literal, in bytes.
pub const MAX_STRING: usize = 1 << 20;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),
    // keywords
    Let,
    Fn,
    If,
    Else,
    While,
    For,
    In,
    Return,
    Break,
    Continue,
    True,
    False,
    Nil,
    And,
    Or,
    Not,
    // punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semi,
    Newline,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub line: u32,
}

pub fn lex(src: &str) -> Result<Vec<Token>, ScriptError> {
    if src.len() > MAX_SOURCE {
        return Err(ScriptError::new(0, "script is too large"));
    }
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut line = 1u32;
    let at = |i: usize| chars.get(i).copied();
    while let Some(c) = at(i) {
        match c {
            ' ' | '\t' | '\r' => i += 1,
            '\n' => {
                out.push(Token {
                    tok: Tok::Newline,
                    line,
                });
                line += 1;
                i += 1;
            }
            '#' => {
                while matches!(at(i), Some(ch) if ch != '\n') {
                    i += 1;
                }
            }
            '"' => {
                i += 1;
                let mut s = String::new();
                loop {
                    match at(i) {
                        None | Some('\n') => {
                            return Err(ScriptError::new(line, "unterminated string"));
                        }
                        Some('"') => {
                            i += 1;
                            break;
                        }
                        Some('\\') => {
                            let e = at(i + 1)
                                .ok_or_else(|| ScriptError::new(line, "unterminated string"))?;
                            s.push(match e {
                                'n' => '\n',
                                't' => '\t',
                                'r' => '\r',
                                '0' => '\0',
                                '"' => '"',
                                '\\' => '\\',
                                _ => return Err(ScriptError::new(line, "unknown escape")),
                            });
                            i += 2;
                        }
                        Some(ch) => {
                            s.push(ch);
                            i += 1;
                        }
                    }
                    if s.len() > MAX_STRING {
                        return Err(ScriptError::new(line, "string literal is too long"));
                    }
                }
                out.push(Token {
                    tok: Tok::Str(s),
                    line,
                });
            }
            c if c.is_ascii_digit() => {
                let start = i;
                while matches!(at(i), Some(ch) if ch.is_ascii_digit() || ch == '_') {
                    i += 1;
                }
                let mut float = false;
                if at(i) == Some('.') && matches!(at(i + 1), Some(ch) if ch.is_ascii_digit()) {
                    float = true;
                    i += 1;
                    while matches!(at(i), Some(ch) if ch.is_ascii_digit() || ch == '_') {
                        i += 1;
                    }
                }
                if matches!(at(i), Some('e' | 'E')) {
                    let mut j = i + 1;
                    if matches!(at(j), Some('+' | '-')) {
                        j += 1;
                    }
                    if matches!(at(j), Some(ch) if ch.is_ascii_digit()) {
                        float = true;
                        i = j;
                        while matches!(at(i), Some(ch) if ch.is_ascii_digit()) {
                            i += 1;
                        }
                    }
                }
                let text: String = chars
                    .get(start..i)
                    .unwrap_or(&[])
                    .iter()
                    .filter(|c| **c != '_')
                    .collect();
                let tok = if float {
                    match text.parse::<f64>() {
                        Ok(v) if v.is_finite() => Tok::Float(v),
                        _ => return Err(ScriptError::new(line, "bad number")),
                    }
                } else {
                    match text.parse::<i64>() {
                        Ok(v) => Tok::Int(v),
                        Err(_) => return Err(ScriptError::new(line, "integer is too large")),
                    }
                };
                out.push(Token { tok, line });
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while matches!(at(i), Some(ch) if ch.is_alphanumeric() || ch == '_') {
                    i += 1;
                }
                let word: String = chars.get(start..i).unwrap_or(&[]).iter().collect();
                let tok = match word.as_str() {
                    "let" => Tok::Let,
                    "fn" => Tok::Fn,
                    "if" => Tok::If,
                    "else" => Tok::Else,
                    "while" => Tok::While,
                    "for" => Tok::For,
                    "in" => Tok::In,
                    "return" => Tok::Return,
                    "break" => Tok::Break,
                    "continue" => Tok::Continue,
                    "true" => Tok::True,
                    "false" => Tok::False,
                    "nil" => Tok::Nil,
                    "and" => Tok::And,
                    "or" => Tok::Or,
                    "not" => Tok::Not,
                    _ => Tok::Ident(word),
                };
                out.push(Token { tok, line });
            }
            _ => {
                let next = at(i + 1);
                let (tok, len) = match (c, next) {
                    ('=', Some('=')) => (Tok::Eq, 2),
                    ('!', Some('=')) => (Tok::Ne, 2),
                    ('<', Some('=')) => (Tok::Le, 2),
                    ('>', Some('=')) => (Tok::Ge, 2),
                    ('+', Some('=')) => (Tok::PlusAssign, 2),
                    ('-', Some('=')) => (Tok::MinusAssign, 2),
                    ('*', Some('=')) => (Tok::StarAssign, 2),
                    ('/', Some('=')) => (Tok::SlashAssign, 2),
                    ('(', _) => (Tok::LParen, 1),
                    (')', _) => (Tok::RParen, 1),
                    ('{', _) => (Tok::LBrace, 1),
                    ('}', _) => (Tok::RBrace, 1),
                    ('[', _) => (Tok::LBracket, 1),
                    (']', _) => (Tok::RBracket, 1),
                    (',', _) => (Tok::Comma, 1),
                    (':', _) => (Tok::Colon, 1),
                    (';', _) => (Tok::Semi, 1),
                    ('+', _) => (Tok::Plus, 1),
                    ('-', _) => (Tok::Minus, 1),
                    ('*', _) => (Tok::Star, 1),
                    ('/', _) => (Tok::Slash, 1),
                    ('%', _) => (Tok::Percent, 1),
                    ('=', _) => (Tok::Assign, 1),
                    ('<', _) => (Tok::Lt, 1),
                    ('>', _) => (Tok::Gt, 1),
                    _ => {
                        return Err(ScriptError::new(
                            line,
                            format!("unexpected character {c:?}"),
                        ))
                    }
                };
                out.push(Token { tok, line });
                i += len;
            }
        }
    }
    out.push(Token {
        tok: Tok::Eof,
        line,
    });
    Ok(out)
}
