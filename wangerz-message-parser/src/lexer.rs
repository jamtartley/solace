#![allow(dead_code)]

#[derive(Clone, Debug, PartialEq)]
pub struct TextSpan(usize, usize);

impl From<(TextSpan, TextSpan)> for TextSpan {
    fn from(value: (TextSpan, TextSpan)) -> Self {
        // @REFACTOR: TextSpan is stupid lol
        Self(value.0 .0, value.1 .1)
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
            return Token::new(TokenKind::Eof, TextSpan(self.current_pos, self.current_pos));
        }

        match self.current() {
            '/' if self.current_pos == 0 => self.lex_special('/'),
            '@' | '#'
                if self.current_pos == 0 || self.content[self.current_pos - 1].is_whitespace() =>
            {
                self.lex_special(self.current())
            }
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
        // @BUG: specials should be preceded by whitespace and terminate at non-alpha
        assert!(['/', '@', '#'].contains(&marker));

        let start = self.current_pos;

        self.advance();

        while !self.is_at_end() && self.current().is_alphanumeric() {
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
                    TextSpan(0, 9)
                ),
                Token::new(TokenKind::Text(" some text".to_owned()), TextSpan(9, 19)),
                Token::new(TokenKind::Eof, TextSpan(19, 19))
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
                Token::new(TokenKind::Text("hello ".to_owned()), TextSpan(0, 6)),
                Token::new(
                    TokenKind::UserMention("@username".to_owned()),
                    TextSpan(6, 15)
                ),
                Token::new(
                    TokenKind::Text(", how are you?".to_owned()),
                    TextSpan(15, 29)
                ),
                Token::new(TokenKind::Eof, TextSpan(29, 29))
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
                    TextSpan(0, 17)
                ),
                Token::new(TokenKind::Eof, TextSpan(17, 17))
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
                    TextSpan(0, 8)
                ),
                Token::new(TokenKind::Text(" some text".to_owned()), TextSpan(8, 18)),
                Token::new(TokenKind::Eof, TextSpan(18, 18))
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
                Token::new(TokenKind::Text("to ".to_owned()), TextSpan(0, 3)),
                Token::new(
                    TokenKind::ChannelMention("#channel".to_owned()),
                    TextSpan(3, 11)
                ),
                Token::new(TokenKind::Text(": welcome!".to_owned()), TextSpan(11, 21)),
                Token::new(TokenKind::Eof, TextSpan(21, 21))
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
                    TextSpan(0, 17)
                ),
                Token::new(TokenKind::Eof, TextSpan(17, 17))
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
                Token::new(TokenKind::UserMention("@user".to_owned()), TextSpan(0, 5)),
                Token::new(TokenKind::Text("! and ".to_owned()), TextSpan(5, 11)),
                Token::new(
                    TokenKind::ChannelMention("#channel".to_owned()),
                    TextSpan(11, 19)
                ),
                Token::new(TokenKind::Text(".".to_owned()), TextSpan(19, 20)),
                Token::new(TokenKind::Eof, TextSpan(20, 20))
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
                Token::new(TokenKind::Command("/command".to_owned()), TextSpan(0, 8)),
                Token::new(TokenKind::Text(" some text".to_owned()), TextSpan(8, 18)),
                Token::new(TokenKind::Eof, TextSpan(18, 18))
            ]
        );
    }

    #[test]
    fn test_command_not_at_start() {
        let mut lexer = Lexer::new("This is not /command");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(
                    TokenKind::Text("This is not /command".to_owned()),
                    TextSpan(0, 20)
                ),
                Token::new(TokenKind::Eof, TextSpan(20, 20))
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
                Token::new(TokenKind::Command("/command".to_owned()), TextSpan(0, 8)),
                Token::new(TokenKind::Text("! follow up".to_owned()), TextSpan(8, 19)),
                Token::new(TokenKind::Eof, TextSpan(19, 19))
            ]
        );
    }

    #[test]
    fn test_multiple_commands() {
        let mut lexer = Lexer::new("/start then /middle and /end");
        let tokens = lexer.lex();
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenKind::Command("/start".to_owned()), TextSpan(0, 6)),
                Token::new(
                    TokenKind::Text(" then /middle and /end".to_owned()),
                    TextSpan(6, 28)
                ),
                Token::new(TokenKind::Eof, TextSpan(28, 28))
            ]
        );
    }
}
