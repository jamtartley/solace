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
    Whitespace {
        span: TextSpan,
    },
}

impl AstNode {
    fn contains_pos(&self, pos: usize) -> bool {
        match self {
            AstNode::Command { span, .. } => span.contains(pos),
            AstNode::UserMention { span, .. } => span.contains(pos),
            AstNode::ChannelMention { span, .. } => span.contains(pos),
            AstNode::Text { span, .. } => span.contains(pos),
            AstNode::Whitespace { span } => span.contains(pos),
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
            TokenKind::Whitespace(_) => (Some(AstNode::Whitespace { span }), 1),
            TokenKind::Eof => (None, 0),
        };

        parser.advance(consumed_len);

        next
    }
}

#[cfg(test)]
mod tests {}
