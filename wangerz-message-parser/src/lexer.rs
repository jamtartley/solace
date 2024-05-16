#![allow(dead_code)]

#[derive(Clone, Debug)]
pub(crate) enum Token {
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
            '/' if self.current_pos == 0 => self.lex_command(),
            '@' => self.lex_user_mention(),
            '#' => self.lex_channel_mention(),
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

    fn lex_command(&mut self) -> Option<Token> {
        self.advance();

        let start = self.current_pos;
        while !(self.is_at_end() || self.current().is_whitespace()) {
            self.advance();
        }

        Some(Token::Command(
            self.content[start..self.current_pos].iter().collect(),
        ))
    }

    fn lex_user_mention(&mut self) -> Option<Token> {
        todo!()
    }

    fn lex_channel_mention(&mut self) -> Option<Token> {
        todo!()
    }

    fn lex_text(&mut self) -> Option<Token> {
        let start = self.current_pos;

        while !(self.is_at_end() || self.is_start_of_special()) {
            self.advance();
        }

        Some(Token::Text(
            self.content[start..self.current_pos].iter().collect(),
        ))
    }

    fn is_start_of_special(&self) -> bool {
        match self.current() {
            '@' | '#' => true,
            _ => false,
        }
    }
}
