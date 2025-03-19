use std::{
    fs,
    io::{self, Write as _},
};

use anyhow::Context as _;

pub(crate) fn write_and_flush_buf<T: io::Write>(w: &mut T, buf: &[u8]) -> anyhow::Result<()> {
    let mut buf = buf.to_owned();
    buf.push(b'\n');

    w.write_all(&buf).context("failed to write output")?;
    w.flush().context("failed to flush output")
}

pub(crate) fn write_and_flush_str<T: io::Write>(w: &mut T, s: &str) -> anyhow::Result<()> {
    write_and_flush_buf(w, s.as_bytes())
}

pub(crate) fn prompt_and_read() -> anyhow::Result<String> {
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
