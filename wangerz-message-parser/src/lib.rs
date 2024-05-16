pub struct Message;

mod lexer;

pub fn parse(message: &str) -> Message {
    let mut lexer = lexer::Lexer::new(message);

    while let Some(token) = lexer.get_next_token() {
        println!("{:?}", token);
    }

    Message {}
}
