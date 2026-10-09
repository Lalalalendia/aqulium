use super::exclusions::is_excluded_term;
use super::sanitize::sanitize_for_search;
use tantivy::tokenizer::{SimpleTokenizer, Token, TokenFilter, TokenStream, Tokenizer};

#[derive(Clone, Default)]
pub struct SanitizingTokenizer {
    inner: SimpleTokenizer,
    buffer: String,
}

impl Tokenizer for SanitizingTokenizer {
    type TokenStream<'a> = <SimpleTokenizer as Tokenizer>::TokenStream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        self.buffer = sanitize_for_search(text);
        self.inner.token_stream(&self.buffer)
    }
}

#[derive(Clone, Default)]
pub struct ExclusionFilter;

impl TokenFilter for ExclusionFilter {
    type Tokenizer<T: Tokenizer> = ExclusionTokenizer<T>;

    fn transform<T: Tokenizer>(self, tokenizer: T) -> Self::Tokenizer<T> {
        ExclusionTokenizer { inner: tokenizer }
    }
}

#[derive(Clone)]
pub struct ExclusionTokenizer<T> {
    inner: T,
}

impl<T: Tokenizer> Tokenizer for ExclusionTokenizer<T> {
    type TokenStream<'a> = ExclusionTokenStream<T::TokenStream<'a>>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        ExclusionTokenStream {
            tail: self.inner.token_stream(text),
        }
    }
}

pub struct ExclusionTokenStream<T> {
    tail: T,
}

impl<T: TokenStream> TokenStream for ExclusionTokenStream<T> {
    fn advance(&mut self) -> bool {
        while self.tail.advance() {
            let text = &self.tail.token().text;
            if text.is_empty() || is_excluded_term(text) {
                continue;
            }
            if !text.chars().any(|ch| ch.is_alphanumeric()) {
                continue;
            }
            return true;
        }
        false
    }

    fn token(&self) -> &Token {
        self.tail.token()
    }

    fn token_mut(&mut self) -> &mut Token {
        self.tail.token_mut()
    }
}
