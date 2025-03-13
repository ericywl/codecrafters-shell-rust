use std::io::{self, Write};

use anyhow::Context;

mod builtin;

fn write_output_and_flush(buf: &[u8]) -> anyhow::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(&buf).context("failed to write output")?;
    stdout
        .write_all("\n".as_bytes())
        .context("failed to write newline")?;
    stdout.flush().context("failed to flush output")
}

fn prompt_and_read_input() -> anyhow::Result<String> {
    let mut stdout = io::stdout();
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
    Ok(input)
}

pub fn repl() -> anyhow::Result<()> {
    loop {
        let input = prompt_and_read_input()?;

        // Parse command
        let mut iter = input.split_whitespace();
        let command = iter.next();
        let args: Vec<_> = iter.collect();
        let command = match command {
            Some(command) => builtin::Command::parse(command),
            None => continue,
        };

        // Execute command
        command.execute(&args)?;
    }
}
