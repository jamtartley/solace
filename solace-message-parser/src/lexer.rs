#![allow(dead_code)]

use std::iter::Peekable;

use unicode_segmentation::{Graphemes, UnicodeSegmentation};

macro_rules! token {
    ($k: expr, $c0: expr, $c1: expr) => {
        Token::new($k, TextSpan::new($c0, $c1))
    };
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextSpan {
    pub c0: usize,
    pub c1: usize,
}

impl TextSpan {
    pub(crate) fn new(c0: usize, c1: usize) -> Self {
        Self { c0, c1 }
    }

    pub fn len(&self) -> usize {
        self.c1 - self.c0
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains(&self, pos: usize) -> bool {
        self.c0 <= pos && pos <= self.c1
    }
}

impl From<(TextSpan, TextSpan)> for TextSpan {
    fn from(value: (TextSpan, TextSpan)) -> Self {
        Self {
            c0: value.0.c0,
            c1: value.1.c1,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) span: TextSpan,
}

impl Token {
    fn new(kind: TokenKind, span: TextSpan) -> Self {
        Self { kind, span }
    }
}

#[derive(Clone, Debug, PartialEq)]

pub(crate) enum TokenKind {
    Text(String),
    Command(String),
    UserMention(String),
    ChannelMention(String),
    Whitespace(usize),
    Eof,
}

pub(crate) struct Lexer<'a> {
    pub(crate) tokens: Vec<Token>,

    content: Peekable<Graphemes<'a>>,
    len: usize,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(content: &'a str) -> Self {
        Self {
            tokens: vec![],

            content: content.graphemes(true).peekable(),
            len: content.len(),
            pos: 0,
        }
    }

    pub(crate) fn lex(&mut self) -> Vec<Token> {
        loop {
            // @FIXME: Logic still isn't right here...
            // Splitting on whitespace will lead to things
            // like `hello @user!` being split into `@user!`
            // but it should be @user
            if let Some(token) = self.eat_whitespace() {
                self.tokens.push(token);
            }

            let start = self.pos;
            let kind = match self.current() {
                Some("/") => TokenKind::Command,
                Some("@") => TokenKind::UserMention,
                Some("#") => TokenKind::ChannelMention,
                Some(_) => TokenKind::Text,
                None => break,
            };

            let word = self.consume_word();
            let len = word.chars().count();

            self.tokens.push(token!(kind(word), start, start + len));
        }

        self.tokens.push(token!(TokenKind::Eof, self.pos, self.pos));

        self.tokens.clone()
    }

    fn current(&mut self) -> Option<&str> {
        self.content.peek().copied()
    }

    fn advance(&mut self) {
        if self.pos < self.len {
            let _ = self.content.next();
            self.pos += 1;
        }
    }

    fn consume_word(&mut self) -> String {
        let mut s = String::new();

        loop {
            match self.current() {
                Some(str) if !str.trim().is_empty() => s.push_str(str),
                _ => break,
            }

            self.advance();
        }

        s
    }

    fn eat_whitespace(&mut self) -> Option<Token> {
        let start = self.pos;
        let mut len = 0;

        loop {
            match self.current() {
                Some(s) if s.trim().is_empty() => {
                    len += 1;
                    self.advance();
                }
                _ => {
                    if len > 0 {
                        return Some(token!(TokenKind::Whitespace(len), start, start + len));
                    } else {
                        return None;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {}
