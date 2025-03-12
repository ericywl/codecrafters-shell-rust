use std::io::{self, Write};

use anyhow::Context;
use strum::EnumString;

mod builtin;

#[derive(Debug, PartialEq, EnumString)]
enum Command {
    #[strum(serialize = "exit")]
    Exit,

    #[strum(serialize = "echo")]
    Echo,

    #[strum(disabled)]
    Custom(String),
}

impl Command {
    fn parse(command: &str) -> Self {
        match Command::try_from(command) {
            Ok(cmd) => cmd,
            Err(_) => Self::Custom(command.to_owned()),
        }
    }

    fn execute(&self, args: &[&str]) -> anyhow::Result<()> {
        match self {
            Self::Exit => builtin::exit(args),
            Self::Echo => !todo!(),
            Self::Custom(c) => Command::command_not_found(c),
        }
    }

    fn command_not_found(command: &str) -> anyhow::Result<()> {
        write_output_and_flush(format!("{command}: command not found").into())
    }
}

fn write_output_and_flush(mut buf: Vec<u8>) -> anyhow::Result<()> {
    buf.push(b'\n');
    let mut stdout = io::stdout();
    stdout.write_all(&buf).context("failed to write output")?;
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
            Some(command) => Command::parse(command),
            None => continue,
        };

        // Execute command
        command.execute(&args)?;
    }
}
