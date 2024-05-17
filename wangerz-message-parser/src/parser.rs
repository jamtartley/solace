#![allow(dead_code)]

use crate::lexer::{Lexer, TextSpan, Token, TokenKind};

#[derive(Clone, Debug, Default)]
pub struct Ast {
    pub nodes: Vec<AstNode>,
}

#[derive(Clone, Debug)]
pub enum AstNode {
    Command {
        span: TextSpan,
        name: String,
        args: Vec<AstNode>,
    },
    UserMention {
        span: TextSpan,
        user_name: String,
    },
    ChannelMention {
        span: TextSpan,
        channel_name: String,
    },
    Text {
        span: TextSpan,
        value: String,
    },
}

pub struct Parser {
    pub ast: Ast,

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
            ast: Ast::default(),
            current_pos: 0,
            tokens,
        }
    }

    pub fn parse(&mut self) -> Ast {
        Ast::parse(self).unwrap()
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

impl Parse for Ast {
    fn parse(parser: &mut Parser) -> Option<Self> {
        let mut nodes = vec![];

        while !parser.is_at_eof() {
            if let Some(node) = AstNode::parse(parser) {
                nodes.push(node);
            }
        }

        Some(Self { nodes })
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
                        name,
                        args: args.clone(),
                    }),
                    0,
                )
            }
            TokenKind::UserMention(value) => (
                Some(AstNode::UserMention {
                    span,
                    user_name: value,
                }),
                1,
            ),
            TokenKind::ChannelMention(value) => (
                Some(AstNode::ChannelMention {
                    span,
                    channel_name: value,
                }),
                1,
            ),
            TokenKind::Eof => (None, 0),
        };

        parser.advance(consumed_len);

        next
    }
}
