pub mod expr;
pub mod query;

use super::ast::Query;
use super::lexer::{tokenize, Piece, Symbol, Token};

pub fn parse(text: &str) -> Result<Query, String> {
    let tokens = tokenize(text)?;
    let mut cursor = Cursor::new(text, &tokens);
    let parsed = query::query(&mut cursor)?;
    if let Some(token) = cursor.peek() {
        let found = cursor.text_of(token);
        let hint = match token.piece {
            Piece::Word(_) => ". Если это ещё одна колонка, перед ней нужна запятая",
            _ => "",
        };
        return Err(format!("Лишнее в запросе после разбора: «{found}»{hint}"));
    }
    Ok(parsed)
}

pub struct Cursor<'a> {
    text: &'a str,
    tokens: &'a [Token],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(text: &'a str, tokens: &'a [Token]) -> Self {
        Self {
            text,
            tokens,
            position: 0,
        }
    }

    pub fn mark(&self) -> usize {
        self.position
    }

    pub fn rewind(&mut self, mark: usize) {
        self.position = mark;
    }

    pub fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.position)
    }

    pub fn peek_after(&self) -> Option<&'a Token> {
        self.tokens.get(self.position + 1)
    }

    pub fn advance(&mut self) -> Option<&'a Token> {
        let token = self.tokens.get(self.position)?;
        self.position += 1;
        Some(token)
    }

    pub fn text_of(&self, token: &Token) -> &'a str {
        &self.text[token.from..token.to]
    }

    pub fn slice(&self, from: usize, to: usize) -> &'a str {
        self.text[from..to].trim()
    }

    pub fn offset(&self) -> usize {
        self.peek()
            .map(|token| token.from)
            .unwrap_or(self.text.len())
    }

    pub fn previous_end(&self) -> usize {
        self.tokens
            .get(self.position.wrapping_sub(1))
            .map(|token| token.to)
            .unwrap_or(0)
    }

    pub fn word(&self) -> Option<&'a str> {
        match &self.peek()?.piece {
            Piece::Word(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub fn at_word(&self, name: &str) -> bool {
        self.word()
            .is_some_and(|found| found.eq_ignore_ascii_case(name))
    }

    pub fn take_word(&mut self, name: &str) -> bool {
        if !self.at_word(name) {
            return false;
        }
        self.position += 1;
        true
    }

    pub fn at_symbol(&self, symbol: Symbol) -> bool {
        matches!(self.peek().map(|token| &token.piece), Some(Piece::Symbol(found)) if *found == symbol)
    }

    pub fn take_symbol(&mut self, symbol: Symbol) -> bool {
        if !self.at_symbol(symbol) {
            return false;
        }
        self.position += 1;
        true
    }

    pub fn expect_symbol(&mut self, symbol: Symbol, expected: &str) -> Result<(), String> {
        if self.take_symbol(symbol) {
            return Ok(());
        }
        Err(match self.peek() {
            Some(token) => format!("Ожидается {expected}, а не «{}»", self.text_of(token)),
            None => format!("Запрос кончился, а ожидается {expected}"),
        })
    }
}
