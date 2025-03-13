use std::{
    env, fs,
    path::PathBuf,
    process::{self},
};

use strum::EnumString;

use crate::write_output_and_flush;

#[derive(Debug, PartialEq, EnumString)]
pub(crate) enum Command {
    #[strum(serialize = "exit")]
    Exit,

    #[strum(serialize = "echo")]
    Echo,

    #[strum(serialize = "type")]
    Type,

    #[strum(disabled)]
    Executable { name: String },
}

impl Command {
    pub(crate) fn parse(command: &str) -> Self {
        match Self::try_from(command) {
            Ok(cmd) => cmd,
            Err(_) => Self::Executable {
                name: command.to_owned(),
            },
        }
    }

    pub(crate) fn execute(&self, args: &[&str]) -> anyhow::Result<()> {
        match self {
            Self::Exit => Self::exit(args),
            Self::Echo => Self::echo(args),
            Self::Type => Self::type_cmd(args),
            Self::Executable { name } => Self::command_not_found(name),
        }
    }

    fn exit(args: &[&str]) -> anyhow::Result<()> {
        let code = match args.first() {
            Some(arg) => match arg.parse::<i32>() {
                Ok(c) => c,
                Err(_) => 0,
            },
            None => 0,
        };

        process::exit(code)
    }

    fn echo(args: &[&str]) -> anyhow::Result<()> {
        write_output_and_flush(args.join(" ").into())
    }

    fn type_cmd(args: &[&str]) -> anyhow::Result<()> {
        let mut outputs = Vec::new();
        for &arg in args {
            let output = match Self::parse(arg) {
                Self::Executable { name } => match Self::find_executable_in_path(&name) {
                    Some(path) => format!("{name} is {}", path.display()),
                    None => format!("{name}: not found"),
                },
                _ => format!("{arg} is a shell builtin"),
            };
            outputs.push(output);
        }

        write_output_and_flush(outputs.join("\n").into())?;
        Ok(())
    }

    fn command_not_found(command: &str) -> anyhow::Result<()> {
        write_output_and_flush(format!("{command}: command not found").into())
    }

    fn find_executable_in_path(name: &str) -> Option<PathBuf> {
        let path_env_var = env::var("PATH");
        if path_env_var.is_err() {
            return None;
        }

        let path_env_var = path_env_var.unwrap();
        let splits = path_env_var.split(":");

        for p in splits {
            let entries = match fs::read_dir(p) {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            for entry in entries {
                let path = match entry {
                    Ok(entry) => entry.path(),
                    Err(_) => continue,
                };

                if !path.is_file() {
                    continue;
                }

                match path.file_name() {
                    Some(filename) => {
                        if filename == name {
                            return Some(path);
                        }
                    }
                    None => continue,
                }
            }
        }

        None
    }
}
