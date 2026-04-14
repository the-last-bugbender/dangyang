use crate::error::ParseError;

/// A single token produced by the YANG lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// An unquoted word: keyword or bare identifier.
    Word(String),
    /// A quoted string (already unescaped and concatenated).
    Str(String),
    LBrace,
    RBrace,
    Semicolon,
}

/// Tokenises `source` into a flat `Vec<(Token, usize)>` where the `usize`
/// is the byte offset of the token start.
pub fn tokenise(source: &str) -> Result<Vec<(Token, usize)>, ParseError> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut pos = 0;
    let mut tokens = Vec::new();

    while pos < len {
        // Skip whitespace
        if bytes[pos].is_ascii_whitespace() {
            pos += 1;
            continue;
        }

        // Line comment
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'/' {
            while pos < len && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }

        // Block comment
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            pos += 2;
            loop {
                if pos + 1 >= len {
                    return Err(ParseError::UnterminatedString(pos));
                }
                if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                    pos += 2;
                    break;
                }
                pos += 1;
            }
            continue;
        }

        let start = pos;

        match bytes[pos] {
            b'{' => {
                tokens.push((Token::LBrace, start));
                pos += 1;
            }
            b'}' => {
                tokens.push((Token::RBrace, start));
                pos += 1;
            }
            b';' => {
                tokens.push((Token::Semicolon, start));
                pos += 1;
            }

            // Quoted strings — may be concatenated with `+`
            b'"' | b'\'' => {
                let s = collect_string(bytes, &mut pos, len)?;
                // Handle concatenation
                let tok_start = start;
                let mut combined = s;
                loop {
                    let saved = pos;
                    // skip whitespace
                    let mut p2 = pos;
                    while p2 < len && bytes[p2].is_ascii_whitespace() {
                        p2 += 1;
                    }
                    if p2 < len && bytes[p2] == b'+' {
                        p2 += 1;
                        // skip whitespace again
                        while p2 < len && bytes[p2].is_ascii_whitespace() {
                            p2 += 1;
                        }
                        if p2 < len && (bytes[p2] == b'"' || bytes[p2] == b'\'') {
                            pos = p2;
                            let next = collect_string(bytes, &mut pos, len)?;
                            combined.push_str(&next);
                            continue;
                        }
                    }
                    let _ = saved;
                    break;
                }
                tokens.push((Token::Str(combined), tok_start));
            }

            // Unquoted word: identifier characters + module prefix colon
            _ if is_word_char(bytes[pos]) => {
                while pos < len && is_word_char(bytes[pos]) {
                    pos += 1;
                }
                let word = source[start..pos].to_string();
                tokens.push((Token::Word(word), start));
            }

            other => {
                return Err(ParseError::UnexpectedToken {
                    pos: start,
                    expected: "a valid YANG token",
                    got: format!("byte 0x{other:02X}"),
                });
            }
        }
    }

    Ok(tokens)
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':' | b'/')
}

/// Parse a single quoted or double-quoted string starting at `bytes[*pos]`.
fn collect_string(bytes: &[u8], pos: &mut usize, len: usize) -> Result<String, ParseError> {
    let start = *pos;
    let quote = bytes[*pos];
    *pos += 1;
    let mut s = String::new();

    if quote == b'\'' {
        // Single-quoted: no escape sequences
        loop {
            if *pos >= len {
                return Err(ParseError::UnterminatedString(start));
            }
            if bytes[*pos] == b'\'' {
                *pos += 1;
                return Ok(s);
            }
            s.push(bytes[*pos] as char);
            *pos += 1;
        }
    } else {
        // Double-quoted: supports \n \t \\ \"
        loop {
            if *pos >= len {
                return Err(ParseError::UnterminatedString(start));
            }
            match bytes[*pos] {
                b'"' => {
                    *pos += 1;
                    return Ok(s);
                }
                b'\\' => {
                    *pos += 1;
                    if *pos >= len {
                        return Err(ParseError::UnterminatedString(start));
                    }
                    let esc = match bytes[*pos] {
                        b'n' => '\n',
                        b't' => '\t',
                        b'\\' => '\\',
                        b'"' => '"',
                        other => return Err(ParseError::InvalidEscape(other as char)),
                    };
                    s.push(esc);
                    *pos += 1;
                }
                b => {
                    s.push(b as char);
                    *pos += 1;
                }
            }
        }
    }
}
