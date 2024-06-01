#![allow(dead_code)]

use crate::lexer::{Lexer, TextSpan, Token, TokenKind};

#[derive(Clone, Debug, PartialEq)]
pub enum AstMessage {
    Command(AstNode),
    Normal(Vec<AstNode>),
}

impl Default for AstMessage {
    fn default() -> Self {
        Self::Normal(vec![])
    }
}

impl AstMessage {
    pub fn node_at_pos(&self, pos: usize) -> Option<&AstNode> {
        match self {
            AstMessage::Command(command) => command.contains_pos(pos).then_some(command),
            AstMessage::Normal(nodes) => nodes.iter().find(|n| n.contains_pos(pos)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AstNode {
    Command {
        span: TextSpan,
        raw_name: String,
        parsed_name: String,
        args: Vec<AstNode>,
    },
    UserMention {
        span: TextSpan,
        raw_user_name: String,
        parsed_user_name: String,
    },
    ChannelMention {
        span: TextSpan,
        raw_channel_name: String,
        parsed_channel_name: String,
    },
    Text {
        span: TextSpan,
        value: String,
    },
    Whitespace(usize),
}

impl AstNode {
    fn contains_pos(&self, pos: usize) -> bool {
        match self {
            AstNode::Command { span, .. } => span.contains(pos),
            AstNode::UserMention { span, .. } => span.contains(pos),
            AstNode::ChannelMention { span, .. } => span.contains(pos),
            AstNode::Text { span, .. } => span.contains(pos),
            AstNode::Whitespace(_) => false,
        }
    }
}

pub struct Parser {
    pub ast: AstMessage,

    pub current_pos: usize,
    tokens: Vec<Token>,
}

trait Parse
where
    Self: Sized,
{
    fn parse(parser: &mut Parser) -> Option<Self>;
}

impl Parser {
    pub fn new(message: &str) -> Self {
        let tokens = Lexer::new(message).lex();

        assert!(!tokens.is_empty());

        Self {
            ast: AstMessage::default(),
            current_pos: 0,
            tokens,
        }
    }

    pub fn parse(&mut self) -> AstMessage {
        AstMessage::parse(self).unwrap()
    }

    fn current_token(&self) -> Token {
        self.tokens[self.current_pos].clone()
    }

    fn is_at_eof(&self) -> bool {
        self.current_token().kind == TokenKind::Eof
    }

    fn advance(&mut self, n: usize) {
        for _ in 0..n {
            if !self.is_at_eof() {
                self.current_pos += 1;
            }
        }
    }
}

impl Parse for AstMessage {
    fn parse(parser: &mut Parser) -> Option<Self> {
        let mut nodes = vec![];

        while !parser.is_at_eof() {
            if let Some(node) = AstNode::parse(parser) {
                nodes.push(node);
            }
        }

        match nodes.first() {
            Some(AstNode::Command { .. }) => Some(Self::Command(nodes.first().unwrap().clone())),
            Some(_) => Some(Self::Normal(nodes)),
            None => unreachable!(),
        }
    }
}

impl Parse for AstNode {
    fn parse(parser: &mut Parser) -> Option<Self> {
        let Token { span, kind } = parser.current_token();

        let (next, consumed_len) = match kind {
            TokenKind::Text(value) => (Some(AstNode::Text { span, value }), 1),
            TokenKind::Command(value) => {
                let name = value;
                let mut args = vec![];

                parser.advance(1);

                while !parser.is_at_eof() {
                    if let Some(arg) = AstNode::parse(parser) {
                        args.push(arg);
                    }
                }

                (
                    Some(AstNode::Command {
                        span,
                        // @CLEANUP: parsed/raw name cloning
                        raw_name: name.clone(),
                        parsed_name: name.clone()[1..].to_string(),
                        args: args.clone(),
                    }),
                    0,
                )
            }
            TokenKind::UserMention(value) => (
                Some(AstNode::UserMention {
                    span,
                    raw_user_name: value.clone(),
                    parsed_user_name: value.clone()[1..].to_owned(),
                }),
                1,
            ),
            TokenKind::ChannelMention(value) => (
                Some(AstNode::ChannelMention {
                    span,
                    raw_channel_name: value.clone(),
                    parsed_channel_name: value.clone()[1..].to_owned(),
                }),
                1,
            ),
            TokenKind::Whitespace(len) => (Some(AstNode::Whitespace(len)), 1),
            TokenKind::Eof => (None, 0),
        };

        parser.advance(consumed_len);

        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::TextSpan;

    #[test]
    fn test_simple_text_only_message() {
        let mut parser = Parser::new("hello, world!");
        let ast = parser.parse();
        assert_eq!(
            ast,
            AstMessage::Normal(vec![AstNode::Text {
                span: TextSpan::new(0, 13),
                value: "hello, world!".to_owned()
            }])
        );
    }

    #[test]
    fn test_simple_command() {
        let mut parser = Parser::new("/command some text");
        let ast = parser.parse();
        assert_eq!(
            ast,
            AstMessage::Command(AstNode::Command {
                span: TextSpan::new(0, 8),
                raw_name: "/command".to_owned(),
                parsed_name: "command".to_owned(),
                args: vec![AstNode::Text {
                    span: TextSpan::new(8, 18),
                    value: " some text".to_owned()
                }]
            })
        );
    }

    #[test]
    fn test_user_mention() {
        let mut parser = Parser::new("Hello @user!");
        let ast = parser.parse();
        assert_eq!(
            ast,
            AstMessage::Normal(vec![
                AstNode::Text {
                    span: TextSpan::new(0, 6),
                    value: "Hello ".to_owned(),
                },
                AstNode::UserMention {
                    span: TextSpan::new(6, 11),
                    raw_user_name: "@user".to_owned(),
                    parsed_user_name: "user".to_owned(),
                },
                AstNode::Text {
                    span: TextSpan::new(11, 12),
                    value: "!".to_owned(),
                },
            ])
        );
    }

    #[test]
    fn test_channel_mention() {
        let mut parser = Parser::new("Check out #channel now");
        let ast = parser.parse();
        assert_eq!(
            ast,
            AstMessage::Normal(vec![
                AstNode::Text {
                    span: TextSpan::new(0, 10),
                    value: "Check out ".to_owned(),
                },
                AstNode::ChannelMention {
                    span: TextSpan::new(10, 18),
                    raw_channel_name: "#channel".to_owned(),
                    parsed_channel_name: "channel".to_owned(),
                },
                AstNode::Text {
                    span: TextSpan::new(18, 22),
                    value: " now".to_owned(),
                },
            ])
        );
    }

    #[test]
    fn test_channel_and_users_are_parsed_in_command() {
        let mut parser = Parser::new("/start #solace test @user");
        let ast = parser.parse();
        assert_eq!(
            ast,
            AstMessage::Command(AstNode::Command {
                span: TextSpan::new(0, 6),
                raw_name: "/start".to_owned(),
                parsed_name: "start".to_owned(),
                args: vec![
                    AstNode::Text {
                        span: TextSpan::new(6, 7),
                        value: " ".to_owned()
                    },
                    AstNode::ChannelMention {
                        span: TextSpan::new(7, 14),
                        raw_channel_name: "#solace".to_owned(),
                        parsed_channel_name: "solace".to_owned()
                    },
                    AstNode::Text {
                        span: TextSpan::new(14, 20),
                        value: " test ".to_owned()
                    },
                    AstNode::UserMention {
                        span: TextSpan::new(20, 25),
                        raw_user_name: "@user".to_owned(),
                        parsed_user_name: "user".to_owned()
                    },
                ]
            })
        );
    }

    #[test]
    fn test_only_first_command_counts() {
        let mut parser = Parser::new("/start /cmd1 arg1 /cmd2 arg2 arg3 end");
        let ast = parser.parse();
        assert_eq!(
            ast,
            AstMessage::Command(AstNode::Command {
                span: TextSpan::new(0, 6),
                raw_name: "/start".to_owned(),
                parsed_name: "start".to_owned(),
                args: vec![AstNode::Text {
                    span: TextSpan::new(6, 37),
                    value: " /cmd1 arg1 /cmd2 arg2 arg3 end".to_owned()
                }]
            })
        );
    }
}
