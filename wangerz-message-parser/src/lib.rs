pub use parser::{Ast, AstNode, Parser};

mod lexer;
mod parser;

pub fn parse(message: &str) -> Ast {
    let mut parser = Parser::new(message);
    parser.parse()
}
