use std::{
    fmt::format,
    process::{self},
};

use crate::{write_output_and_flush, Command};

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

pub(crate) fn type_cmd(args: &[&str]) -> anyhow::Result<()> {
    let mut outputs = Vec::new();
    for &arg in args {
        let output = match Command::parse(arg) {
            Command::Custom(_) => format!("{arg}: not found"),
            _ => format!("{arg} is a shell builtin"),
        };
        outputs.push(output);
    }

    write_output_and_flush(outputs.join("\n").into())?;
    Ok(())
}
