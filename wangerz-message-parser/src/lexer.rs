#![allow(dead_code)]

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
            return Token::new(
                TokenKind::Eof,
                TextSpan::new(self.current_pos, self.current_pos),
            );
        }

        match self.current() {
            '/' if self.current_pos == 0 => self.lex_special('/', |ch| ch.is_whitespace()),
            '@' | '#'
                if self.current_pos == 0 || self.content[self.current_pos - 1].is_whitespace() =>
            {
                self.lex_special(self.current(), |ch| !ch.is_alphanumeric())
            }
            _ => self.lex_text(),
        }
    }

    fn current(&self) -> char {
        // @FIXME: Panics on multi-byte chars, should probably be using a string for content
        self.content[self.current_pos]
    }

    fn peek(&self) -> Option<char> {
        if self.is_at_end() {
            None
        } else {
            Some(self.content[self.current_pos + 1])
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

    fn lex_special<F>(&mut self, marker: char, terminate_when: F) -> Token
    where
        F: Fn(char) -> bool,
    {
        assert!(['/', '@', '#'].contains(&marker));

        let start = self.current_pos;

        self.advance();

        while !self.is_at_end() && !terminate_when(self.current()) {
            self.advance();
        }

        let value = self.content[start..self.current_pos].iter().collect();
        let span = TextSpan::new(start, self.current_pos);

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
            TextSpan::new(start, self.current_pos),
        )
    }

    fn is_start_of_special(&self) -> bool {
        matches!(self.current(), '@' | '#')
            && (self.current_pos == 0 || self.content[self.current_pos - 1].is_whitespace())
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
