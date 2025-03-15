use std::io::{self, Write};

use anyhow::Context;

mod builtin;

fn write_and_flush_buf(buf: &[u8]) -> anyhow::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(&buf).context("failed to write output")?;
    stdout
        .write_all("\n".as_bytes())
        .context("failed to write newline")?;
    stdout.flush().context("failed to flush output")
}

fn write_and_flush_str(s: &str) -> anyhow::Result<()> {
    write_and_flush_buf(s.as_bytes())
}

fn prompt_and_read() -> anyhow::Result<String> {
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
        let input = prompt_and_read()?;
        let (command_str, args_str) = match input.split_once(char::is_whitespace) {
            Some(splits) => splits,
            None => continue,
        };

        // Parse and execute
        let command = builtin::Command::parse(command_str.trim());
        match parse_args(args_str) {
            Ok(args) => command.execute(&args)?,
            Err(e) => write_and_flush_str(&e)?,
        };
    }
}

fn parse_args(args_str: &str) -> Result<Vec<String>, String> {
    let args_str = args_str.trim();
    let mut args: Vec<String> = Vec::new();
    let mut in_single_quotes = false;
    let mut prev_end_quote_idx: Option<usize> = None;
    let mut start_idx = 0;
    for (idx, c) in args_str.char_indices() {
        match c {
            '\'' => {
                in_single_quotes = !in_single_quotes;
                if in_single_quotes {
                    start_idx = idx;
                } else {
                    let arg = &args_str[start_idx + 1..idx];
                    if !arg.is_empty() {
                        match prev_end_quote_idx {
                            Some(peq_idx) => {
                                if peq_idx == start_idx - 1 {
                                    let len = args.len();
                                    args[len - 1].push_str(&arg);
                                } else {
                                    args.push(arg.to_owned())
                                }
                            }
                            None => args.push(arg.to_owned()),
                        };
                        prev_end_quote_idx = Some(idx);
                    }
                    start_idx = idx + 1;
                }
            }
            _ if c.is_whitespace() => {
                if !in_single_quotes {
                    let arg = &args_str[start_idx..idx];
                    if !arg.is_empty() {
                        args.push(arg.to_owned());
                    }
                    start_idx = idx + 1;
                }
            }
            _ => (),
        }
    }

    if in_single_quotes {
        return Err("quotes unfinished".into());
    }

    let arg = &args_str[start_idx..];
    if !arg.trim().is_empty() {
        args.push(arg.to_owned());
    }

    Ok(args)
}

#[cfg(test)]
mod test {
    use crate::parse_args;

    #[test]
    fn test_parse_args_trailing_whitespace() {
        let args = parse_args("script  shell  ");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["script", "shell"]);
    }

    #[test]
    fn test_parse_args_whitespace_between() {
        let args = parse_args("script    shell");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["script", "shell"]);
    }

    #[test]
    fn test_parse_args_whitespace_in_quotes() {
        let args = parse_args("'script    shell'");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["script    shell"]);
    }

    #[test]
    fn test_parse_args_whitespace_between_two_quotes() {
        let args = parse_args("' script '   ' shell '");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![" script ", " shell "]);
    }

    #[test]
    fn test_parse_args_no_space_between_two_quotes() {
        let args = parse_args("' script''shell'");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![" scriptshell"]);
    }
}
