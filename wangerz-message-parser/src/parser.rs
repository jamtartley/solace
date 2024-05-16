#![allow(dead_code)]

use crate::lexer::{Lexer, TextSpan, Token};

#[derive(Clone, Debug, Default)]
pub struct Ast {
    nodes: Vec<AstNode>,
}

#[derive(Clone, Debug)]
pub enum AstNode {
    Command {
        span: TextSpan,
        name: String,
        args: Vec<String>,
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

    current_pos: usize,
    tokens: Vec<Token>,
}

trait Parse
where
    Self: Sized,
{
    fn parse(parser: &mut Parser) -> Self;
}

impl Parser {
    pub fn new(message: &str) -> Self {
        let tokens = Lexer::new(message).lex();

        assert!(tokens.len() > 0);
        println!("tokens: {tokens:?}");

        Self {
            ast: Ast::default(),
            current_pos: 0,
            tokens,
        }
    }

    pub fn parse(&mut self) -> Ast {
        Ast::parse(self)
    }
}

impl Parse for Ast {
    fn parse(parser: &mut Parser) -> Self {
        let mut nodes = vec![];

        Self { nodes }
    }
}
