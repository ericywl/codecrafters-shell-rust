use std::{
    fs,
    io::{self, Write as _},
};

use anyhow::Context as _;
use rustyline::error::ReadlineError;

pub(crate) fn write_and_flush_buf<T: io::Write>(w: &mut T, buf: &[u8]) -> anyhow::Result<()> {
    let mut buf = buf.to_owned();
    buf.push(b'\n');

    w.write_all(&buf).context("failed to write output")?;
    w.flush().context("failed to flush output")
}

pub(crate) fn write_and_flush_str<T: io::Write>(w: &mut T, s: &str) -> anyhow::Result<()> {
    write_and_flush_buf(w, s.as_bytes())
}

pub(crate) fn prompt_and_readline<H, I>(rl: &mut rustyline::Editor<H, I>) -> anyhow::Result<String>
where
    H: rustyline::Helper,
    I: rustyline::history::History,
{
    let readline = rl.readline("$ ");
    let input = match readline {
        Ok(line) => line,
        Err(ReadlineError::Interrupted) => {
            return Ok("".into());
        }
        Err(ReadlineError::Eof) => return Err(anyhow::anyhow!("<CTRL-D>")),
        Err(err) => return Err(anyhow::anyhow!("failed to readline: {}", err)),
    };

    Ok(input)
}

pub(crate) fn redirect_to(redirects: &[&str], buf: &[u8]) -> anyhow::Result<()> {
    for r in redirects {
        match fs::File::create(r) {
            Ok(mut file) => {
                let res = file.write_all(buf);
                if res.is_err() {
                    write_and_flush_str(
                        &mut io::stderr(),
                        &format!("failed to write to file {r}: {}", res.unwrap_err()),
                    )?;
                }
            }
            Err(e) => write_and_flush_str(
                &mut io::stderr(),
                &format!("failed to create file {r}: {e}"),
            )?,
        };
    }

    Ok(())
}

pub(crate) fn append_to(appends: &[&str], buf: &[u8]) -> anyhow::Result<()> {
    for a in appends {
        match fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(a)
        {
            Ok(mut file) => {
                let res = file.write_all(buf);
                if res.is_err() {
                    write_and_flush_str(
                        &mut io::stderr(),
                        &format!("failed to append to file {a}: {}", res.unwrap_err()),
                    )?;
                }
            }
            Err(e) => {
                write_and_flush_str(&mut io::stderr(), &format!("failed to open file {a}: {e}"))?
            }
        };
    }

    Ok(())
}
