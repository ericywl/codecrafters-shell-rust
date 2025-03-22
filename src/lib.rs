use std::io::{self, Write};

use anyhow::Context;
use builtin::Output;
use rustyline::{Completer, Helper, Highlighter, Hinter, Validator};

mod builtin;
mod util;

pub fn repl() -> anyhow::Result<()> {
    let completer = ShellCompleter;
    let helper = ShellHelper { completer };
    let mut rl = rustyline::Editor::new().context("failed to create new rustyline editor")?;
    rl.set_helper(Some(helper));

    loop {
        // Read input
        let input = match util::prompt_and_readline(&mut rl)? {
            Some(input) => input,
            None => return Ok(()),
        };

        // Tokenize the input
        let tokens = match tokenize(&input) {
            Ok(tokens) => tokens,
            Err(e) => {
                util::write_and_flush_str(&mut io::stderr(), &e)?;
                continue;
            }
        };

        // Split commands and redirects
        let split = match split_tokens(tokens.as_ref()) {
            Ok(s) => s,
            Err(e) => {
                util::write_and_flush_str(&mut io::stderr(), &e)?;
                continue;
            }
        };

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
        redirect_and_append(split, &out_buf, &err_buf)?;
    }
}

#[derive(Completer, Helper, Highlighter, Hinter, Validator)]
struct ShellHelper {
    #[rustyline(Completer)]
    completer: ShellCompleter,
}

struct ShellCompleter;

impl rustyline::completion::Completer for ShellCompleter {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        _: usize,
        _: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let words = builtin::Command::available_commands();
        let completions = words
            .iter()
            .filter(|w| w.starts_with(line))
            .map(|s| s.to_string() + " ")
            .collect();
        Ok((0, completions))
    }
}

fn redirect_and_append(split: Split<'_>, out_buf: &[u8], err_buf: &[u8]) -> anyhow::Result<()> {
    if split.outs.len() > 0 {
        util::redirect_to(&split.outs, &out_buf)?;
    }
    if split.append_outs.len() > 0 {
        util::append_to(&split.append_outs, &out_buf)?;
    }
    if split.outs.len() == 0 && split.append_outs.len() == 0 {
        io::stdout()
            .write_all(&out_buf)
            .context("failed to write output")?;
    }
    if split.errs.len() > 0 {
        util::redirect_to(&split.errs, &err_buf)?;
    }
    if split.append_errs.len() > 0 {
        util::append_to(&split.append_errs, &err_buf)?;
    }
    if split.errs.len() == 0 && split.append_errs.len() == 0 {
        io::stderr()
            .write_all(&err_buf)
            .context("failed to write errors")?;
    }

    Ok(())
}

struct Split<'a> {
    cmd_args: Vec<&'a str>,
    outs: Vec<&'a str>,
    append_outs: Vec<&'a str>,
    errs: Vec<&'a str>,
    append_errs: Vec<&'a str>,
}

impl<'a> Split<'a> {
    fn new() -> Self {
        Self {
            cmd_args: Vec::new(),
            outs: Vec::new(),
            append_outs: Vec::new(),
            errs: Vec::new(),
            append_errs: Vec::new(),
        }
    }
}

enum Redirect {
    Out,
    AppendOut,
    Err,
    AppendErr,
}

fn split_tokens<T: AsRef<str>>(tokens: &[T]) -> Result<Split, String> {
    let mut split = Split::new();
    let mut redirect: Option<Redirect> = None;

    for token in tokens {
        let token = token.as_ref();
        match token {
            // Two redirects at once, which is invalid.
            "1>" | ">" | "1>>" | ">>" | "2>" | "2>>" => {
                if redirect.is_some() {
                    return Err(format!("parse error near {token}"));
                }
            }
            _ => (),
        }

        match token {
            "1>" | ">" => redirect = Some(Redirect::Out),
            "1>>" | ">>" => redirect = Some(Redirect::AppendOut),
            "2>" => redirect = Some(Redirect::Err),
            "2>>" => redirect = Some(Redirect::AppendErr),
            _ => {
                match redirect {
                    Some(r) => match r {
                        Redirect::Out => split.outs.push(token),
                        Redirect::AppendOut => split.append_outs.push(token),
                        Redirect::Err => split.errs.push(token),
                        Redirect::AppendErr => split.append_errs.push(token),
                    },
                    None => split.cmd_args.push(token),
                }
                redirect = None;
            }
        }
    }

    Ok(split)
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
        let split = split_tokens(&tokens).unwrap();
        assert_eq!(split.cmd_args, vec!["echo", "hello", "world"]);
        assert!(split.outs.is_empty());
        assert!(split.append_outs.is_empty());
        assert!(split.errs.is_empty());
        assert!(split.append_errs.is_empty());
    }

    #[test]
    fn test_redirect_out() {
        let tokens = vec!["echo", "hello", "world", ">", "/tmp/data"];
        let split = split_tokens(&tokens).unwrap();
        assert_eq!(split.cmd_args, vec!["echo", "hello", "world"]);
        assert_eq!(split.outs, vec!["/tmp/data"]);
        assert!(split.append_outs.is_empty());
        assert!(split.errs.is_empty());
        assert!(split.append_errs.is_empty());
    }

    #[test]
    fn test_multiple_redirect_outs() {
        let tokens = vec!["echo", "thisistest", ">", "/tmp/data", ">", "./a/b"];
        let split = split_tokens(&tokens).unwrap();
        assert_eq!(split.cmd_args, vec!["echo", "thisistest"]);
        assert_eq!(split.outs, vec!["/tmp/data", "./a/b"]);
        assert!(split.append_outs.is_empty());
        assert!(split.errs.is_empty());
        assert!(split.append_errs.is_empty());
    }

    #[test]
    fn test_mutliple_redirect_errs() {
        let tokens = vec![
            "echo",
            "big bad error",
            "2>",
            "./error.log",
            "2>",
            "./warn.log",
        ];
        let split = split_tokens(&tokens).unwrap();
        assert_eq!(split.cmd_args, vec!["echo", "big bad error"]);
        assert!(split.outs.is_empty());
        assert!(split.append_outs.is_empty());
        assert_eq!(split.errs, vec!["./error.log", "./warn.log"]);
        assert!(split.append_errs.is_empty());
    }

    #[test]
    fn test_mixed_redirect() {
        let tokens = vec![
            "cat",
            "./something.txt",
            ">",
            "/tmp/data",
            ">>",
            "/tmp/extra_data",
            "2>",
            "./error.log",
            "2>>",
            "dump",
        ];
        let split = split_tokens(&tokens).unwrap();
        assert_eq!(split.cmd_args, vec!["cat", "./something.txt"]);
        assert_eq!(split.outs, vec!["/tmp/data"]);
        assert_eq!(split.append_outs, vec!["/tmp/extra_data"]);
        assert_eq!(split.errs, vec!["./error.log"]);
        assert_eq!(split.append_errs, vec!["dump"]);
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
