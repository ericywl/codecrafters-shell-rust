#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    print!("$ ");
    io::stdout().flush().unwrap();

    // Wait for user input
    let stdin = io::stdin();
    let mut input = String::new();
    stdin.read_line(&mut input).unwrap();

    let mut iter = input.split_whitespace();
    let command = iter.next();
    match command {
        Some(command) => command_not_found(&command),
        None => !todo!(),
    }
}

fn command_not_found(command: &str) {
    let mut stdout = io::stdout();
    stdout
        .write_all(format!("{command}: command not found").as_bytes())
        .unwrap();
}
