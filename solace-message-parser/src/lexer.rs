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
    c0: usize,
    c1: usize,
}

impl TextSpan {
    pub(crate) fn new(c0: usize, c1: usize) -> Self {
        Self { c0, c1 }
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
            self.eat_whitespace();

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

    fn eat_whitespace(&mut self) {
        loop {
            match self.current() {
                Some(s) if s.trim().is_empty() => {
                    self.advance();
                }
                _ => return,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_mention_at_start() {
        let mut lexer = Lexer::new("@username some text");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(
                    TokenKind::UserMention("@username".to_owned()),
                    TextSpan::new(0, 9)
                ),
                Token::new(
                    TokenKind::Text(" some text".to_owned()),
                    TextSpan::new(9, 19)
                ),
                Token::new(TokenKind::Eof, TextSpan::new(19, 19))
            ]
        );
    }

    #[test]
    fn test_user_mention_after_whitespace() {
        let mut lexer = Lexer::new("hello @username, how are you?");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenKind::Text("hello ".to_owned()), TextSpan::new(0, 6)),
                Token::new(
                    TokenKind::UserMention("@username".to_owned()),
                    TextSpan::new(6, 15)
                ),
                Token::new(
                    TokenKind::Text(", how are you?".to_owned()),
                    TextSpan::new(15, 29)
                ),
                Token::new(TokenKind::Eof, TextSpan::new(29, 29))
            ]
        );
    }

    #[test]
    fn test_user_mention_not_preceded_by_whitespace() {
        let mut lexer = Lexer::new("email@example.com");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(
                    TokenKind::Text("email@example.com".to_owned()),
                    TextSpan::new(0, 17)
                ),
                Token::new(TokenKind::Eof, TextSpan::new(17, 17))
            ]
        );
    }

    #[test]
    fn test_channel_mention_at_start() {
        let mut lexer = Lexer::new("#channel some text");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(
                    TokenKind::ChannelMention("#channel".to_owned()),
                    TextSpan::new(0, 8)
                ),
                Token::new(
                    TokenKind::Text(" some text".to_owned()),
                    TextSpan::new(8, 18)
                ),
                Token::new(TokenKind::Eof, TextSpan::new(18, 18))
            ]
        );
    }

    #[test]
    fn test_channel_mention_after_whitespace() {
        let mut lexer = Lexer::new("to #channel: welcome!");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenKind::Text("to ".to_owned()), TextSpan::new(0, 3)),
                Token::new(
                    TokenKind::ChannelMention("#channel".to_owned()),
                    TextSpan::new(3, 11)
                ),
                Token::new(
                    TokenKind::Text(": welcome!".to_owned()),
                    TextSpan::new(11, 21)
                ),
                Token::new(TokenKind::Eof, TextSpan::new(21, 21))
            ]
        );
    }

    #[test]
    fn test_channel_mention_not_preceded_by_whitespace() {
        let mut lexer = Lexer::new("topic#channelName");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(
                    TokenKind::Text("topic#channelName".to_owned()),
                    TextSpan::new(0, 17)
                ),
                Token::new(TokenKind::Eof, TextSpan::new(17, 17))
            ]
        );
    }

    #[test]
    fn test_non_alphanumeric_termination() {
        let mut lexer = Lexer::new("@user! and #channel.");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(
                    TokenKind::UserMention("@user".to_owned()),
                    TextSpan::new(0, 5)
                ),
                Token::new(TokenKind::Text("! and ".to_owned()), TextSpan::new(5, 11)),
                Token::new(
                    TokenKind::ChannelMention("#channel".to_owned()),
                    TextSpan::new(11, 19)
                ),
                Token::new(TokenKind::Text(".".to_owned()), TextSpan::new(19, 20)),
                Token::new(TokenKind::Eof, TextSpan::new(20, 20))
            ]
        );
    }

    #[test]
    fn test_command_at_start() {
        let mut lexer = Lexer::new("/command some text");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(
                    TokenKind::Command("/command".to_owned()),
                    TextSpan::new(0, 8)
                ),
                Token::new(
                    TokenKind::Text(" some text".to_owned()),
                    TextSpan::new(8, 18)
                ),
                Token::new(TokenKind::Eof, TextSpan::new(18, 18))
            ]
        );
    }

    #[test]
    fn test_command_not_at_start() {
        let mut lexer = Lexer::new("This is not a /command");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(
                    TokenKind::Text("This is not a /command".to_owned()),
                    TextSpan::new(0, 22)
                ),
                Token::new(TokenKind::Eof, TextSpan::new(22, 22))
            ]
        );
    }

    #[test]
    fn test_command_with_trailing_non_alphanumeric() {
        let mut lexer = Lexer::new("/command! follow up");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(
                    TokenKind::Command("/command!".to_owned()),
                    TextSpan::new(0, 9)
                ),
                Token::new(
                    TokenKind::Text(" follow up".to_owned()),
                    TextSpan::new(9, 19)
                ),
                Token::new(TokenKind::Eof, TextSpan::new(19, 19))
            ]
        );
    }

    #[test]
    fn test_only_first_command_counts() {
        let mut lexer = Lexer::new("/start then /middle and /end");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenKind::Command("/start".to_owned()), TextSpan::new(0, 6)),
                Token::new(
                    TokenKind::Text(" then /middle and /end".to_owned()),
                    TextSpan::new(6, 28)
                ),
                Token::new(TokenKind::Eof, TextSpan::new(28, 28))
            ]
        );
    }
}
