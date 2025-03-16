use std::{
    fs,
    io::{self, Write},
};

use anyhow::Context;
use builtin::Output;

mod builtin;

fn write_and_flush_buf<T: io::Write>(w: &mut T, buf: &[u8]) -> anyhow::Result<()> {
    let mut buf = buf.to_owned();
    buf.push(b'\n');

    w.write_all(&buf).context("failed to write output")?;
    w.flush().context("failed to flush output")
}

fn write_and_flush_str<T: io::Write>(w: &mut T, s: &str) -> anyhow::Result<()> {
    write_and_flush_buf(w, s.as_bytes())
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

        // Tokenize the input
        let tokens = match tokenize(&input) {
            Ok(tokens) => tokens,
            Err(e) => return write_and_flush_str(&mut io::stderr(), &e),
        };

        // Split commands and redirects
        let split = split_tokens(tokens.as_ref());

        // Parse command and execute with arguments
        let (command, args) = match split.cmd_args.split_first() {
            Some(ca) => ca,
            None => continue,
        };
        let command = builtin::Command::parse(command);
        // Output to buffers so that we can redirect them
        let (mut out_buf, mut err_buf) = (Vec::new(), Vec::new());
        command.execute(&mut Output::new(&mut out_buf, &mut err_buf), args)?;

        // Redirection, otherwise write to stdout / stderr
        if split.outs.len() > 0 {
            redirect_to(&split.outs, &out_buf)?;
        } else {
            io::stdout()
                .write_all(&out_buf)
                .context("failed to write output")?;
        }
        if split.errs.len() > 0 {
            redirect_to(&split.errs, &err_buf)?;
        } else {
            io::stderr()
                .write_all(&err_buf)
                .context("failed to write errors")?;
        }
    }
}

fn redirect_to(redirects: &[&str], buf: &[u8]) -> anyhow::Result<()> {
    for r in redirects {
        match fs::File::create(r) {
            Ok(mut file) => {
                let res = file.write_all(buf);
                if res.is_err() {
                    write_and_flush_str(
                        &mut io::stderr(),
                        &format!("failed to create file {r}: {}", res.unwrap_err()),
                    )?;
                }
            }
            Err(e) => {
                write_and_flush_str(
                    &mut io::stderr(),
                    &format!("failed to create file {r}: {e}"),
                )?;
            }
        };
    }

    Ok(())
}

struct Split<'a> {
    cmd_args: Vec<&'a str>,
    outs: Vec<&'a str>,
    errs: Vec<&'a str>,
}

impl<'a> Split<'a> {
    fn new() -> Self {
        Self {
            cmd_args: Vec::new(),
            outs: Vec::new(),
            errs: Vec::new(),
        }
    }
}

fn split_tokens<T: AsRef<str>>(tokens: &[T]) -> Split {
    let mut split = Split::new();
    let mut is_out = false;
    let mut is_err = false;
    for token in tokens {
        let token = token.as_ref();
        match token {
            ">" | "1>" => is_out = true,
            "2>" => is_err = true,
            _ => {
                if is_out {
                    split.outs.push(token);
                    is_out = false;
                } else if is_err {
                    split.errs.push(token);
                    is_err = false;
                } else {
                    split.cmd_args.push(token);
                }
            }
        }
    }

    split
}

fn tokenize(input: &str) -> Result<Vec<String>, String> {
    let input = input.trim();
    let mut tokens: Vec<String> = Vec::new();
    let mut next = String::new();
    let mut next_start_idx = 0;
    let mut in_single_quotes = false;
    let mut in_double_quotes = false;
    let mut prev_end_quote_idx: Option<usize> = None;
    let mut escaped = false;

    for (idx, ch) in input.char_indices() {
        match ch {
            // If double quote is in single quotes or escaped, treat it as per normal
            '"' if !in_single_quotes && !escaped => {
                in_double_quotes = !in_double_quotes;
                if in_double_quotes {
                    // Ignore the starting double quote
                    next_start_idx = idx;
                    continue;
                }
                push_next_arg(
                    &mut tokens,
                    &mut next,
                    next_start_idx,
                    prev_end_quote_idx.as_ref(),
                );
                prev_end_quote_idx = Some(idx);
                next_start_idx = idx + 1;
            }
            // If single quote is in double quotes or escaped, treat it as per normal
            '\'' if !in_double_quotes && !escaped => {
                in_single_quotes = !in_single_quotes;
                if in_single_quotes {
                    // Ignore the ending double quote
                    next_start_idx = idx;
                    continue;
                }
                push_next_arg(
                    &mut tokens,
                    &mut next,
                    next_start_idx,
                    prev_end_quote_idx.as_ref(),
                );
                prev_end_quote_idx = Some(idx);
                next_start_idx = idx + 1;
            }
            // If char is not whitespace or is escaped, treat is as per normal
            _ if (ch.is_whitespace() && !escaped) => {
                if in_single_quotes || in_double_quotes {
                    next.push(ch);
                    continue;
                }
                push_next_arg(
                    &mut tokens,
                    &mut next,
                    next_start_idx,
                    prev_end_quote_idx.as_ref(),
                );
                next_start_idx = idx + 1;
            }
            // If backslash is escaped or in single quotes, treat it as per normal char
            '\\' if !escaped && !in_single_quotes => escaped = true,
            // Escaped chars in double quotes have special handling
            _ if escaped && in_double_quotes => {
                escaped = false;
                match ch {
                    // These chars will be escaped
                    '\\' | '$' | '\n' | '"' => next.push(ch),
                    // The rest won't, so we need to restore the backslash
                    _ => {
                        next.push('\\');
                        next.push(ch);
                    }
                };
            }
            // Normal char
            _ => {
                if escaped {
                    escaped = false
                }
                next.push(ch);
            }
        }
    }

    if in_single_quotes || in_double_quotes {
        return Err("quotes unfinished".into());
    }

    push_next_arg(
        &mut tokens,
        &mut next,
        next_start_idx,
        prev_end_quote_idx.as_ref(),
    );
    Ok(tokens)
}

fn push_next_arg(
    args: &mut Vec<String>,
    next_arg: &mut String,
    next_arg_start_idx: usize,
    prev_end_quote_idx: Option<&usize>,
) {
    if next_arg.is_empty() {
        return;
    }
    match prev_end_quote_idx {
        // Combine two quoted strings e.g.
        // `"hello"'world'` => `helloworld`
        Some(&peq_idx) => {
            if peq_idx == next_arg_start_idx - 1 {
                let len = args.len();
                args[len - 1].push_str(next_arg);
                *next_arg = String::new();
            } else {
                args.push(next_arg.clone());
                *next_arg = String::new();
            }
        }
        None => {
            args.push(next_arg.clone());
            *next_arg = String::new();
        }
    };
}

#[cfg(test)]
mod split_test {
    use crate::split_tokens;

    #[test]
    fn test_only_command() {
        let tokens = vec!["echo", "hello", "world"];
        let split = split_tokens(&tokens);
        assert_eq!(split.cmd_args, vec!["echo", "hello", "world"]);
        assert!(split.outs.is_empty());
        assert!(split.errs.is_empty());
    }

    #[test]
    fn test_redirect() {
        let tokens = vec!["echo", "hello", "world", ">", "/tmp/data"];
        let split = split_tokens(&tokens);
        assert_eq!(split.cmd_args, vec!["echo", "hello", "world"]);
        assert_eq!(split.outs, vec!["/tmp/data"]);
        assert!(split.errs.is_empty());
    }

    #[test]
    fn test_multiple_redirects() {
        let tokens = vec!["echo", "thisistest", ">", "/tmp/data", ">", "./a/b"];
        let split = split_tokens(&tokens);
        assert_eq!(split.cmd_args, vec!["echo", "thisistest"]);
        assert_eq!(split.outs, vec!["/tmp/data", "./a/b"]);
        assert!(split.errs.is_empty());
    }
}

#[cfg(test)]
mod tokenize_test {
    use crate::tokenize;

    #[test]
    fn test_trailing_whitespace() {
        let args = tokenize("script  shell  ");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["script", "shell"]);
    }

    #[test]
    fn test_whitespace_between() {
        let args = tokenize("script    shell");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["script", "shell"]);
    }

    #[test]
    fn test_single_quoted() {
        let args = tokenize("'script    shell'");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["script    shell"]);
    }

    #[test]
    fn test_whitespace_between_single_quoteds() {
        let args = tokenize("' script '   ' shell '");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![" script ", " shell "]);
    }

    #[test]
    fn test_no_space_between_single_quoteds() {
        let args = tokenize("' script''shell'");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![" scriptshell"]);
    }

    #[test]
    fn test_no_space_between_single_quoted_and_normal() {
        let args = tokenize("'script'shell");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["scriptshell"]);
    }

    #[test]
    fn test_double_quoted() {
        let args = tokenize(r#""quz  hello"  "bar""#);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["quz  hello", "bar"]);
    }

    #[test]
    fn test_no_space_between_double_quoted_and_normal() {
        let args = tokenize("\"script\"shell");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["scriptshell"]);
    }

    #[test]
    fn test_single_quoted_in_double_quoted() {
        let args = tokenize("\"'quz''hello'\"");
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec!["'quz''hello'"]);
    }

    #[test]
    fn test_backslash() {
        let args = tokenize(r#"world\ \ \ \\\ \ \ script"#);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![r#"world   \   script"#]);
    }

    #[test]
    fn test_backslash_in_single_quoted() {
        let args = tokenize(r#"'example\"testhello\"shell'"#);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![r#"example\"testhello\"shell"#]);
    }

    #[test]
    fn test_backslash_in_double_quoted() {
        let args = tokenize(r#""hello'script'\\n'world""#);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![r#"hello'script'\n'world"#]);
    }

    #[test]
    fn test_backslash_before_quotes() {
        let args = tokenize(r#""hello\"insidequotes"script\""#);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![r#"hello"insidequotesscript""#]);
    }

    #[test]
    fn test_backslash_before_newline_in_double_quoted() {
        let args = tokenize(r#""hello'script'\\n'world""#);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(args, vec![r#"hello'script'\n'world"#]);
    }

    #[test]
    fn test_backslash_in_single_quoted_in_double_quoted() {
        let args = tokenize(r#""/tmp/foo/'f 46'" "/tmp/foo/'f  \80'" "/tmp/foo/'f \84\'""#);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert_eq!(
            args,
            vec![
                r#"/tmp/foo/'f 46'"#,
                r#"/tmp/foo/'f  \80'"#,
                r#"/tmp/foo/'f \84\'"#
            ]
        );
    }
}
