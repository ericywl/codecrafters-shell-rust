use std::process::{self};

use crate::write_output_and_flush;

pub(crate) fn exit(args: &[&str]) -> anyhow::Result<()> {
    let code = match args.first() {
        Some(arg) => match arg.parse::<i32>() {
            Ok(c) => c,
            Err(_) => 0,
        },
        None => 0,
    };

    process::exit(code)
}

pub(crate) fn echo(args: &[&str]) -> anyhow::Result<()> {
    write_output_and_flush(args.join(" ").into())
}
