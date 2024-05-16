use parser::{Ast, Parser};

mod lexer;
mod parser;

pub fn parse(message: &str) -> Ast {
    let mut parser = Parser::new(message);
    parser.parse()
}
