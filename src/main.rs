#[allow(unused_imports)]
use std::io::{self, Write};

use anyhow::Context;

fn main() -> anyhow::Result<()> {
    repl()
}

fn repl() -> anyhow::Result<()> {
    loop {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all("$ ".as_bytes())
            .context("failed to write prompt")?;
        stdout.flush().context("failed to flush prompt")?;

        // Wait for user input
        let stdin = io::stdin();
        let mut input = String::new();
        stdin
            .read_line(&mut input)
            .context("failed to read input")?;

        // Parse command
        let mut iter = input.split_whitespace();
        let command = iter.next();

        // Execute command
        let mut output = match command {
            Some(command) => command_not_found(&command),
            None => continue,
        };

        // Write output
        output.push(b'\n');
        stdout
            .write_all(&output)
            .context("failed to write output")?;
        stdout.flush().context("failed to flush output")?;
    }
}

fn command_not_found(command: &str) -> Vec<u8> {
    format!("{command}: command not found").into()
}
