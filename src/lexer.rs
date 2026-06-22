pub struct Lexer {
    input: String,
    pos: usize,
}

impl Lexer {
    pub fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.pos += character.len_utf8();
        Some(character)
    }

    fn lex_literal(&mut self, mut literal: String) -> Token {
        while let Some(character) = self.peek() {
            if character == '%' {
                break;
            }
            literal.push(character);
            self.advance();
        }

        Token::Literal(literal)
    }

    fn lex_identifier(&mut self, marker_position: usize) -> Result<Token, LexError> {
        let Some(first) = self.peek() else {
            return Err(LexError::EmptyIdentifier {
                position: marker_position,
            });
        };
        if !is_identifier_start(first) {
            return Err(LexError::InvalidIdentifierStart {
                position: marker_position,
                found: first,
            });
        }

        let mut identifier = String::new();
        identifier.push(first);
        self.advance();

        while let Some(character) = self.peek() {
            if !is_identifier_continue(character) {
                break;
            }
            identifier.push(character);
            self.advance();
        }

        Ok(Token::Identifier(identifier))
    }

    fn next_token(&mut self) -> Result<Option<Token>, LexError> {
        let Some(character) = self.peek() else {
            return Ok(None);
        };

        if character != '%' {
            return Ok(Some(self.lex_literal(String::new())));
        }

        let marker_position = self.pos;
        self.advance();
        if self.peek() == Some('%') {
            self.advance();
            return Ok(Some(self.lex_literal("%".into())));
        }

        self.lex_identifier(marker_position).map(Some)
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        while let Some(token) = self.next_token()? {
            tokens.push(token);
        }

        Ok(tokens)
    }
}

fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

fn is_identifier_continue(character: char) -> bool {
    is_identifier_start(character) || character.is_ascii_digit()
}

#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    Identifier(String),
    Literal(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum LexError {
    EmptyIdentifier { position: usize },
    InvalidIdentifierStart { position: usize, found: char },
}
