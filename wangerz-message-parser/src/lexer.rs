#![allow(dead_code)]

#[derive(Clone, Debug)]
pub struct TextSpan(usize, usize);

impl From<(TextSpan, TextSpan)> for TextSpan {
    fn from(value: (TextSpan, TextSpan)) -> Self {
        // @REFACTOR: TextSpan is stupid lol
        Self(value.0 .0, value.1 .1)
    }
}

#[derive(Clone, Debug)]
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
    Eof,
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

    pub(crate) fn lex(&mut self) -> Vec<Token> {
        loop {
            let token = self.get_next_token();
            self.tokens.push(token.clone());

            if token.kind == TokenKind::Eof {
                break;
            }
        }

        self.tokens.clone()
    }

    fn get_next_token(&mut self) -> Token {
        if self.is_at_end() {
            return Token::new(TokenKind::Eof, TextSpan(self.current_pos, self.current_pos));
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

    fn lex_special(&mut self, marker: char) -> Token {
        assert!(vec!['/', '@', '#'].contains(&marker));

        let start = self.current_pos;

        self.advance();

        while !(self.is_at_end() || self.current().is_whitespace()) {
            self.advance();
        }

        let value = self.content[start..self.current_pos].iter().collect();
        let span = TextSpan(start, self.current_pos);

        match marker {
            '/' => Token::new(TokenKind::Command(value), span),
            '@' => Token::new(TokenKind::UserMention(value), span),
            '#' => Token::new(TokenKind::ChannelMention(value), span),
            symbol => panic!("Unexpected symbol: {symbol}"),
        }
    }

    fn lex_text(&mut self) -> Token {
        let start = self.current_pos;

        while !(self.is_at_end() || self.is_start_of_special()) {
            self.advance();
        }

        Token::new(
            TokenKind::Text(self.content[start..self.current_pos].iter().collect()),
            TextSpan(start, self.current_pos),
        )
    }

    fn is_start_of_special(&self) -> bool {
        matches!(self.current(), '@' | '#')
    }
}
