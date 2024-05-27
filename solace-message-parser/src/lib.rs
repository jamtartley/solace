pub use parser::{AstMessage, AstNode, Parser};

mod lexer;
mod parser;

pub fn parse(message: &str) -> AstMessage {
    let mut parser = Parser::new(message);
    parser.parse()
}
