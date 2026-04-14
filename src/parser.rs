use crate::{
    ast::{BitDef, EnumVariant, Restriction, Status, TypeStmt, TypedefNode},
    error::ParseError,
    lexer::{Token, tokenise},
};

/// Parse all `typedef` statements in `source`, skipping everything else.
pub fn parse_typedefs(source: &str) -> Result<Vec<TypedefNode>, ParseError> {
    let tokens = tokenise(source)?;
    let mut p = Parser::new(&tokens);
    p.parse_module()
}

// ---------------------------------------------------------------------------
// Internal parser
// ---------------------------------------------------------------------------

struct Parser<'t> {
    tokens: &'t [(Token, usize)],
    pos: usize,
}

impl<'t> Parser<'t> {
    fn new(tokens: &'t [(Token, usize)]) -> Self {
        Self { tokens, pos: 0 }
    }

    // ---- token helpers ----------------------------------------------------

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    fn peek_pos(&self) -> usize {
        self.tokens.get(self.pos).map(|(_, p)| *p).unwrap_or(0)
    }

    fn advance(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos).map(|(t, _)| t);
        self.pos += 1;
        t
    }

    #[allow(dead_code)]
    fn expect_word(&mut self, kw: &'static str) -> Result<(), ParseError> {
        let pos = self.peek_pos();
        match self.advance().cloned() {
            Some(Token::Word(w)) if w == kw => Ok(()),
            Some(t) => Err(ParseError::UnexpectedToken {
                pos,
                expected: kw,
                got: token_desc(&t),
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    /// Consume and return the string value of the next token, which must be
    /// either a `Word` or a `Str`. In YANG many argument positions accept
    /// either bare identifiers or quoted strings.
    fn expect_string_arg(&mut self) -> Result<String, ParseError> {
        let pos = self.peek_pos();
        match self.advance().cloned() {
            Some(Token::Word(w)) => Ok(w),
            Some(Token::Str(s)) => Ok(s),
            Some(t) => Err(ParseError::UnexpectedToken {
                pos,
                expected: "string argument",
                got: token_desc(&t),
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn expect_lbrace(&mut self) -> Result<(), ParseError> {
        let pos = self.peek_pos();
        match self.advance().cloned() {
            Some(Token::LBrace) => Ok(()),
            Some(t) => Err(ParseError::UnexpectedToken {
                pos,
                expected: "{",
                got: token_desc(&t),
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn expect_semicolon(&mut self) -> Result<(), ParseError> {
        let pos = self.peek_pos();
        match self.advance().cloned() {
            Some(Token::Semicolon) => Ok(()),
            Some(t) => Err(ParseError::UnexpectedToken {
                pos,
                expected: ";",
                got: token_desc(&t),
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    // ---- skip helpers -----------------------------------------------------

    /// Skip a statement that is terminated by `;` or a `{ ... }` block.
    fn skip_statement(&mut self) -> Result<(), ParseError> {
        // We have already consumed the keyword. Now consume the rest.
        loop {
            match self.peek() {
                None => return Err(ParseError::UnexpectedEof),
                Some(Token::Semicolon) => {
                    self.advance();
                    return Ok(());
                }
                Some(Token::LBrace) => {
                    self.advance();
                    self.skip_block()?;
                    return Ok(());
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Skip everything up to and including the matching `}`.
    fn skip_block(&mut self) -> Result<(), ParseError> {
        let mut depth = 1usize;
        loop {
            match self.advance() {
                None => return Err(ParseError::UnexpectedEof),
                Some(Token::LBrace) => depth += 1,
                Some(Token::RBrace) => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
    }

    // ---- top-level --------------------------------------------------------

    /// Parse the top level of a YANG module, collecting any `typedef` blocks.
    fn parse_module(&mut self) -> Result<Vec<TypedefNode>, ParseError> {
        let mut typedefs = Vec::new();

        // Skip optional module/submodule wrapper
        // A real YANG file starts with `module <name> { ... }` or `submodule`.
        // We handle both flat files (just typedefs) and wrapped modules.

        loop {
            match self.peek() {
                None => break,
                Some(Token::RBrace) => {
                    // Closing brace of the module wrapper — consume and stop.
                    self.advance();
                    break;
                }
                Some(Token::Word(w)) => {
                    match w.as_str() {
                        "typedef" => {
                            self.advance();
                            typedefs.push(self.parse_typedef()?);
                        }
                        "module" | "submodule" => {
                            // Consume `module <name> {` and then recurse inside
                            self.advance();
                            let _ = self.expect_string_arg()?; // name
                            self.expect_lbrace()?;
                            // Parse the body recursively
                            let inner = self.parse_module()?;
                            typedefs.extend(inner);
                        }
                        _ => {
                            self.advance();
                            self.skip_statement()?;
                        }
                    }
                }
                Some(t) => {
                    return Err(ParseError::UnexpectedToken {
                        pos: self.peek_pos(),
                        expected: "keyword",
                        got: token_desc(t),
                    });
                }
            }
        }

        Ok(typedefs)
    }

    // ---- typedef ----------------------------------------------------------

    fn parse_typedef(&mut self) -> Result<TypedefNode, ParseError> {
        let name = self.expect_string_arg()?;
        self.expect_lbrace()?;

        let mut type_stmt: Option<TypeStmt> = None;
        let mut description: Option<String> = None;
        let mut units: Option<String> = None;
        let mut default: Option<String> = None;

        loop {
            match self.peek() {
                None => return Err(ParseError::UnexpectedEof),
                Some(Token::RBrace) => {
                    self.advance();
                    break;
                }
                Some(Token::Word(w)) => match w.as_str() {
                    "type" => {
                        self.advance();
                        type_stmt = Some(self.parse_type_stmt()?);
                    }
                    "description" => {
                        self.advance();
                        description = Some(self.expect_string_arg()?);
                        self.expect_semicolon()?;
                    }
                    "units" => {
                        self.advance();
                        units = Some(self.expect_string_arg()?);
                        self.expect_semicolon()?;
                    }
                    "default" => {
                        self.advance();
                        default = Some(self.expect_string_arg()?);
                        self.expect_semicolon()?;
                    }
                    _ => {
                        self.advance();
                        self.skip_statement()?;
                    }
                },
                Some(t) => {
                    return Err(ParseError::UnexpectedToken {
                        pos: self.peek_pos(),
                        expected: "typedef sub-statement",
                        got: token_desc(t),
                    });
                }
            }
        }

        let type_stmt = type_stmt.ok_or(ParseError::UnexpectedToken {
            pos: self.peek_pos(),
            expected: "type statement inside typedef",
            got: "(missing)".into(),
        })?;

        Ok(TypedefNode {
            name,
            type_stmt,
            description,
            units,
            default,
        })
    }

    // ---- type statement ---------------------------------------------------

    fn parse_type_stmt(&mut self) -> Result<TypeStmt, ParseError> {
        let name = self.expect_string_arg()?;
        let restrictions = match self.peek() {
            Some(Token::Semicolon) => {
                self.advance();
                Vec::new()
            }
            Some(Token::LBrace) => {
                self.advance();
                self.parse_type_body(&name)?
            }
            _ => return Err(ParseError::UnexpectedEof),
        };
        Ok(TypeStmt { name, restrictions })
    }

    /// Parse the `{ ... }` body of a `type` statement.
    fn parse_type_body(&mut self, type_name: &str) -> Result<Vec<Restriction>, ParseError> {
        let mut restrictions = Vec::new();

        loop {
            match self.peek() {
                None => return Err(ParseError::UnexpectedEof),
                Some(Token::RBrace) => {
                    self.advance();
                    break;
                }
                Some(Token::Word(w)) => match w.as_str() {
                    "pattern" => {
                        self.advance();
                        let pat = self.expect_string_arg()?;
                        self.expect_semicolon()?;
                        restrictions.push(Restriction::Pattern(pat));
                    }
                    "length" => {
                        self.advance();
                        let expr = self.expect_string_arg()?;
                        self.expect_semicolon()?;
                        restrictions.push(Restriction::Length(expr));
                    }
                    "range" => {
                        self.advance();
                        let expr = self.expect_string_arg()?;
                        self.expect_semicolon()?;
                        restrictions.push(Restriction::Range(expr));
                    }
                    "fraction-digits" => {
                        self.advance();
                        let n_str = self.expect_string_arg()?;
                        let n = n_str
                            .parse::<u8>()
                            .map_err(|_| ParseError::UnexpectedToken {
                                pos: self.peek_pos(),
                                expected: "integer 1..18",
                                got: n_str,
                            })?;
                        self.expect_semicolon()?;
                        restrictions.push(Restriction::FractionDigits(n));
                    }
                    "enum" => {
                        self.advance();
                        restrictions.push(Restriction::Enum(self.parse_enum_variant()?));
                    }
                    "bit" => {
                        self.advance();
                        restrictions.push(Restriction::Bit(self.parse_bit_def()?));
                    }
                    "path" => {
                        self.advance();
                        let path = self.expect_string_arg()?;
                        self.expect_semicolon()?;
                        restrictions.push(Restriction::Path(path));
                    }
                    "require-instance" => {
                        self.advance();
                        let val = self.expect_string_arg()?;
                        let b = match val.as_str() {
                            "true" => true,
                            "false" => false,
                            _ => {
                                return Err(ParseError::UnexpectedToken {
                                    pos: self.peek_pos(),
                                    expected: "true or false",
                                    got: val,
                                });
                            }
                        };
                        self.expect_semicolon()?;
                        restrictions.push(Restriction::RequireInstance(b));
                    }
                    "base" => {
                        self.advance();
                        let base = self.expect_string_arg()?;
                        self.expect_semicolon()?;
                        restrictions.push(Restriction::Base(base));
                    }
                    "type" if type_name == "union" => {
                        self.advance();
                        restrictions.push(Restriction::Type(self.parse_type_stmt()?));
                    }
                    _ => {
                        self.advance();
                        self.skip_statement()?;
                    }
                },
                Some(t) => {
                    return Err(ParseError::UnexpectedToken {
                        pos: self.peek_pos(),
                        expected: "type restriction",
                        got: token_desc(t),
                    });
                }
            }
        }

        Ok(restrictions)
    }

    // ---- enum variant -----------------------------------------------------

    fn parse_enum_variant(&mut self) -> Result<EnumVariant, ParseError> {
        let name = self.expect_string_arg()?;
        let mut value: Option<i64> = None;
        let mut description: Option<String> = None;
        let mut status: Option<Status> = None;

        match self.peek() {
            Some(Token::Semicolon) => {
                self.advance();
            }
            Some(Token::LBrace) => {
                self.advance();
                loop {
                    match self.peek() {
                        None => return Err(ParseError::UnexpectedEof),
                        Some(Token::RBrace) => {
                            self.advance();
                            break;
                        }
                        Some(Token::Word(w)) => match w.as_str() {
                            "value" => {
                                self.advance();
                                let v = self.expect_string_arg()?;
                                let n =
                                    v.parse::<i64>().map_err(|_| ParseError::UnexpectedToken {
                                        pos: self.peek_pos(),
                                        expected: "integer",
                                        got: v,
                                    })?;
                                self.expect_semicolon()?;
                                value = Some(n);
                            }
                            "description" => {
                                self.advance();
                                description = Some(self.expect_string_arg()?);
                                self.expect_semicolon()?;
                            }
                            "status" => {
                                self.advance();
                                status = Some(self.parse_status()?);
                                self.expect_semicolon()?;
                            }
                            _ => {
                                self.advance();
                                self.skip_statement()?;
                            }
                        },
                        Some(t) => {
                            return Err(ParseError::UnexpectedToken {
                                pos: self.peek_pos(),
                                expected: "enum sub-statement",
                                got: token_desc(t),
                            });
                        }
                    }
                }
            }
            _ => return Err(ParseError::UnexpectedEof),
        }

        Ok(EnumVariant {
            name,
            value,
            description,
            status,
        })
    }

    // ---- bit definition ---------------------------------------------------

    fn parse_bit_def(&mut self) -> Result<BitDef, ParseError> {
        let name = self.expect_string_arg()?;
        let mut position: Option<u32> = None;
        let mut description: Option<String> = None;
        let mut status: Option<Status> = None;

        match self.peek() {
            Some(Token::Semicolon) => {
                self.advance();
            }
            Some(Token::LBrace) => {
                self.advance();
                loop {
                    match self.peek() {
                        None => return Err(ParseError::UnexpectedEof),
                        Some(Token::RBrace) => {
                            self.advance();
                            break;
                        }
                        Some(Token::Word(w)) => match w.as_str() {
                            "position" => {
                                self.advance();
                                let v = self.expect_string_arg()?;
                                let n =
                                    v.parse::<u32>().map_err(|_| ParseError::UnexpectedToken {
                                        pos: self.peek_pos(),
                                        expected: "non-negative integer",
                                        got: v,
                                    })?;
                                self.expect_semicolon()?;
                                position = Some(n);
                            }
                            "description" => {
                                self.advance();
                                description = Some(self.expect_string_arg()?);
                                self.expect_semicolon()?;
                            }
                            "status" => {
                                self.advance();
                                status = Some(self.parse_status()?);
                                self.expect_semicolon()?;
                            }
                            _ => {
                                self.advance();
                                self.skip_statement()?;
                            }
                        },
                        Some(t) => {
                            return Err(ParseError::UnexpectedToken {
                                pos: self.peek_pos(),
                                expected: "bit sub-statement",
                                got: token_desc(t),
                            });
                        }
                    }
                }
            }
            _ => return Err(ParseError::UnexpectedEof),
        }

        Ok(BitDef {
            name,
            position,
            description,
            status,
        })
    }

    // ---- status -----------------------------------------------------------

    fn parse_status(&mut self) -> Result<Status, ParseError> {
        let s = self.expect_string_arg()?;
        match s.as_str() {
            "current" => Ok(Status::Current),
            "deprecated" => Ok(Status::Deprecated),
            "obsolete" => Ok(Status::Obsolete),
            _ => Err(ParseError::UnexpectedToken {
                pos: self.peek_pos(),
                expected: "current | deprecated | obsolete",
                got: s,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn token_desc(t: &Token) -> String {
    match t {
        Token::Word(w) => format!("word {w:?}"),
        Token::Str(s) => format!("string {:?}", &s[..s.len().min(40)]),
        Token::LBrace => "{".into(),
        Token::RBrace => "}".into(),
        Token::Semicolon => ";".into(),
    }
}
