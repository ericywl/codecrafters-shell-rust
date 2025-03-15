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
    let mut next_arg = String::new();
    let mut start_idx = 0;
    let mut in_single_quotes = false;
    let mut prev_single_end_quote_idx: Option<usize> = None;
    let mut in_double_quotes = false;
    let mut prev_double_end_quote_idx: Option<usize> = None;
    let mut escaped = false;

    for (idx, ch) in args_str.char_indices() {
        match ch {
            // If backslash is escaped or in single quotes, treat it as per normal char
            '\\' if !escaped && !in_single_quotes && !in_double_quotes => escaped = true,
            // If double quote is in single quotes or escaped, treat it as per normal
            '"' if !in_single_quotes && !escaped => {
                in_double_quotes = !in_double_quotes;
                if in_double_quotes {
                    // Ignore the starting double quote
                    start_idx = idx;
                    continue;
                }
                if !next_arg.is_empty() {
                    match prev_double_end_quote_idx {
                        // Combine two double quoted strings e.g.
                        // `"hello""world"` => `helloworld`
                        Some(pdeq_idx) => {
                            if pdeq_idx == start_idx - 1 {
                                merge_next_arg(&mut args, &mut next_arg);
                            } else {
                                push_next_arg(&mut args, &mut next_arg)
                            }
                        }
                        None => push_next_arg(&mut args, &mut next_arg),
                    };
                    prev_double_end_quote_idx = Some(idx);
                }
                start_idx = idx + 1;
            }
            // If single quote is in double quotes or escaped, treat it as per normal
            '\'' if !in_double_quotes && !escaped => {
                in_single_quotes = !in_single_quotes;
                if in_single_quotes {
                    // Ignore the ending double quote
                    start_idx = idx;
                    continue;
                }
                if !next_arg.is_empty() {
                    match prev_single_end_quote_idx {
                        // Combine two single quoted strings e.g.
                        // `'hello''world'` => `helloworld`
                        Some(pseq_idx) => {
                            if pseq_idx == start_idx - 1 {
                                merge_next_arg(&mut args, &mut next_arg);
                            } else {
                                push_next_arg(&mut args, &mut next_arg);
                            }
                        }
                        None => push_next_arg(&mut args, &mut next_arg),
                    };
                    prev_single_end_quote_idx = Some(idx);
                }
                start_idx = idx + 1;
            }
            // If char is not whitespace or is escaped, treat is as per normal
            _ if (ch.is_whitespace() && !escaped) => {
                if in_single_quotes || in_double_quotes {
                    next_arg.push(ch);
                    continue;
                }
                if !next_arg.is_empty() {
                    push_next_arg(&mut args, &mut next_arg);
                }
                start_idx = idx + 1;
            }
            _ => {
                if escaped {
                    escaped = false
                }
                next_arg.push(ch);
            }
        }
    }

    if in_single_quotes || in_double_quotes {
        return Err("quotes unfinished".into());
    }

    if !next_arg.trim().is_empty() {
        push_next_arg(&mut args, &mut next_arg);
    }

    Ok(args)
}

fn push_next_arg(args: &mut Vec<String>, next_arg: &mut String) {
    args.push(next_arg.clone());
    *next_arg = String::new()
}

fn merge_next_arg(args: &mut Vec<String>, next_arg: &mut String) {
    let len = args.len();
    args[len - 1].push_str(next_arg);
    *next_arg = String::new()
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
    fn test_parse_args_single_quoted() {
        let args = parse_args("'script    shell'");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["script    shell"]);
    }

    #[test]
    fn test_parse_args_whitespace_between_single_quoted() {
        let args = parse_args("' script '   ' shell '");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![" script ", " shell "]);
    }

    #[test]
    fn test_parse_args_no_space_between_single_quoted() {
        let args = parse_args("' script''shell'");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![" scriptshell"]);
    }

    #[test]
    fn test_parse_args_double_quoted() {
        let args = parse_args(r#""quz  hello"  "bar""#);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["quz  hello", "bar"]);
    }

    #[test]
    fn test_parse_args_single_quoted_in_double_quoted() {
        let args = parse_args("\"'quz''hello'\"");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["'quz''hello'"]);
    }

    #[test]
    fn test_parse_args_backslash() {
        let args = parse_args(r#"world\ \ \ \\\ \ \ script"#);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![r#"world   \   script"#]);
    }

    #[test]
    fn test_parse_args_backslash_in_single_quoted() {
        let args = parse_args(r#"'example\"testhello\"shell'"#);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["world\\ \\script"]);
    }

    #[test]
    fn test_parse_args_backslash_in_double_quoted() {
        let args = parse_args(r#""before\   after""#);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![r#"before\   after"#]);
    }

    #[test]
    fn test_parse_args_backslash_before_quotes() {
        let args = parse_args(r#"\'\"hello script\"\'"#);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![r#"'"hello"#, r#"script"'"#]);
    }
}
