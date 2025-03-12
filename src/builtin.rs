use std::process::{self};

pub fn exit(args: &[&str]) -> anyhow::Result<()> {
    let code = match args.first() {
        Some(arg) => match arg.parse::<i32>() {
            Ok(c) => c,
            Err(_) => 0,
        },
        None => 0,
    };

    process::exit(code)
}
