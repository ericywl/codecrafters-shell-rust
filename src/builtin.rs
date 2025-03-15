use std::{
    env, fs,
    path::PathBuf,
    process::{self},
};

use anyhow::Context;
use strum::EnumString;

use crate::{write_and_flush_buf, write_and_flush_str};

#[derive(Debug, PartialEq, EnumString)]
pub(crate) enum Command {
    #[strum(serialize = "exit")]
    Exit,

    #[strum(serialize = "echo")]
    Echo,

    #[strum(serialize = "type")]
    Type,

    #[strum(serialize = "pwd")]
    Pwd,

    #[strum(serialize = "cd")]
    Cd,

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
            Self::Pwd => Self::pwd(args),
            Self::Cd => Self::cd(args),
            Self::Executable { name } => match Self::find_executable_in_path(&name) {
                Some(path) => Self::exec(name, path, args),
                None => Self::command_not_found(&name),
            },
        }
    }

    /// exit terminates the shell with specified code.
    /// If the argument is invalid, code is set to 0 instead.
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

    /// echo prints the same message back.
    fn echo(args: &[&str]) -> anyhow::Result<()> {
        write_and_flush_str(&args.join(" "))
    }

    /// type prints if command is a shell builtin, executable in `$PATH`` or unknown command.
    ///  - If command is a shell builtin: `<command> is a shell builtin`.
    ///  - If command is an executable in PATH: `<command> is <path>`.
    ///  - If command is unknown: `<command>: not found`.
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

        write_and_flush_str(&outputs.join("\n"))?;
        Ok(())
    }

    fn pwd(_: &[&str]) -> anyhow::Result<()> {
        let path = env::current_dir().context("failed to get current dir")?;
        write_and_flush_buf(path.into_os_string().as_encoded_bytes())
    }

    fn cd(args: &[&str]) -> anyhow::Result<()> {
        if args.len() == 0 {
            return Ok(());
        }
        if args.len() > 1 {
            write_and_flush_str("cd: too many arguments")?;
            return Ok(());
        }

        let dir = Self::replace_with_home_dir(args[0]);
        if env::set_current_dir(&dir).is_err() {
            write_and_flush_str(&format!("cd: {}: No such file or directory", dir))?;
        }
        Ok(())
    }

    fn exec(name: &str, path: PathBuf, args: &[&str]) -> anyhow::Result<()> {
        let mut child = process::Command::new(name)
            .args(args)
            .spawn()
            .context(format!(
                "failed to execute program {name} ({})",
                path.display()
            ))?;

        child.wait().context("failed to wait for spawned child")?;
        Ok(())
    }

    fn command_not_found(command: &str) -> anyhow::Result<()> {
        write_and_flush_str(&format!("{command}: command not found"))
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

    fn home_dir() -> String {
        match env::var("HOME") {
            Ok(home) => home,
            Err(_) => "".into(),
        }
    }

    fn replace_with_home_dir(path: &str) -> String {
        match path.split_once('/') {
            Some((a, b)) => {
                if a == "~" {
                    // Replace '~' with HOME dir
                    format!("{}/{}", Self::home_dir(), b)
                } else {
                    path.into()
                }
            }
            None => {
                if path == "~" {
                    Self::home_dir()
                } else {
                    path.into()
                }
            }
        }
    }
}
