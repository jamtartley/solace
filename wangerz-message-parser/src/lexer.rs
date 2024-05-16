#![allow(dead_code)]

#[derive(Clone, Debug)]
pub(crate) struct TextSpan(usize, usize);

#[derive(Clone, Debug)]
pub(crate) struct Token {
    kind: TokenKind,
    span: TextSpan,
}

impl Token {
    fn new(kind: TokenKind, span: TextSpan) -> Self {
        Self { kind, span }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum TokenKind {
    Text(String),
    Command(String),
    Argument(String),
    UserMention(String),
    ChannelMention(String),
}

pub(crate) struct Lexer {
    pub(crate) tokens: Vec<Token>,

    content: Vec<char>,
    current_pos: usize,
    end_pos: usize,
}

impl Lexer {
    pub(crate) fn new(content: &str) -> Self {
        Self {
            tokens: vec![],

            content: content.chars().collect(),
            current_pos: 0,
            end_pos: content.len(),
        }
    }

    pub(crate) fn get_next_token(&mut self) -> Option<Token> {
        if self.is_at_end() {
            return None;
        }

        self.eat_whitespace();

        if self.is_at_end() {
            return None;
        }

        match self.content[self.current_pos] {
            '/' if self.current_pos == 0 => self.lex_special('/'),
            '@' => self.lex_special('@'),
            '#' => self.lex_special('#'),
            _ => self.lex_text(),
        }
    }

    fn current(&self) -> char {
        self.content[self.current_pos]
    }

    fn peek(&self) -> Option<char> {
        match self.is_at_end() {
            false => Some(self.content[self.current_pos + 1]),
            true => None,
        }
    }

    fn is_at_end(&self) -> bool {
        self.current_pos >= self.end_pos
    }

    fn advance(&mut self) {
        if !self.is_at_end() {
            self.current_pos += 1;
        }
    }

    fn eat_whitespace(&mut self) {
        while self.current_pos < self.end_pos && self.current().is_whitespace() {
            self.current_pos += 1;
        }
    }

    fn lex_special(&mut self, marker: char) -> Option<Token> {
        assert!(vec!['/', '@', '#'].contains(&marker));

        self.advance();

        let start = self.current_pos;
        while !(self.is_at_end() || self.current().is_whitespace()) {
            self.advance();
        }

        let value = self.content[start..self.current_pos].iter().collect();
        let span = TextSpan(start, self.current_pos);

        match marker {
            '/' => Some(Token::new(TokenKind::Command(value), span)),
            '@' => Some(Token::new(TokenKind::UserMention(value), span)),
            '#' => Some(Token::new(TokenKind::ChannelMention(value), span)),
            _ => None,
        }
    }

    fn lex_text(&mut self) -> Option<Token> {
        let start = self.current_pos;

        while !(self.is_at_end() || self.is_start_of_special()) {
            self.advance();
        }

        Some(Token::new(
            TokenKind::Text(self.content[start..self.current_pos].iter().collect()),
            TextSpan(start, self.current_pos),
        ))
    }

    fn is_start_of_special(&self) -> bool {
        match self.current() {
            '@' | '#' => true,
            _ => false,
        }
    }
}
