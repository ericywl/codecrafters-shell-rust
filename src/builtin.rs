use std::{
    env, fs, io,
    path::PathBuf,
    process::{self},
};

use anyhow::Context;
use strum::EnumString;

use crate::util::{write_and_flush_buf, write_and_flush_str};

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

pub(crate) struct Output<T, K>
where
    T: io::Write,
    K: io::Write,
{
    out: T,
    err: K,
}

impl<T: io::Write, K: io::Write> Output<T, K> {
    pub(crate) fn new(out: T, err: K) -> Self {
        Self { out, err }
    }
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

    pub(crate) fn execute<T, K>(&self, w: &mut Output<T, K>, args: &[&str]) -> anyhow::Result<()>
    where
        T: io::Write,
        K: io::Write,
    {
        match self {
            Self::Exit => Self::exit(w, args),
            Self::Echo => Self::echo(w, args),
            Self::Type => Self::type_cmd(w, args),
            Self::Pwd => Self::pwd(w, args),
            Self::Cd => Self::cd(w, args),
            Self::Executable { name } => match Self::find_executable_in_path(&name) {
                Some(path) => Self::exec(w, name, path, args),
                None => Self::command_not_found(&mut w.err, &name),
            },
        }
    }

    /// exit terminates the shell with specified code.
    /// If the argument is invalid, code is set to 0 instead.
    fn exit<T, K>(_: &mut Output<T, K>, args: &[&str]) -> anyhow::Result<()>
    where
        T: io::Write,
        K: io::Write,
    {
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
    fn echo<T, K>(w: &mut Output<T, K>, args: &[&str]) -> anyhow::Result<()>
    where
        T: io::Write,
        K: io::Write,
    {
        write_and_flush_str(&mut w.out, &args.join(" "))
    }

    /// type prints if command is a shell builtin, executable in `$PATH`` or unknown command.
    ///  - If command is a shell builtin: `<command> is a shell builtin`.
    ///  - If command is an executable in PATH: `<command> is <path>`.
    ///  - If command is unknown: `<command>: not found`.
    fn type_cmd<T, K>(w: &mut Output<T, K>, args: &[&str]) -> anyhow::Result<()>
    where
        T: io::Write,
        K: io::Write,
    {
        let mut outputs = Vec::new();
        for arg in args {
            let output = match Self::parse(&arg) {
                Self::Executable { name } => match Self::find_executable_in_path(&name) {
                    Some(path) => format!("{name} is {}", path.display()),
                    None => format!("{name}: not found"),
                },
                _ => format!("{arg} is a shell builtin"),
            };
            outputs.push(output);
        }

        write_and_flush_str(&mut w.out, &outputs.join("\n"))?;
        Ok(())
    }

    fn pwd<T, K>(w: &mut Output<T, K>, _: &[&str]) -> anyhow::Result<()>
    where
        T: io::Write,
        K: io::Write,
    {
        let path = env::current_dir().context("failed to get current dir")?;
        write_and_flush_buf(&mut w.out, path.into_os_string().as_encoded_bytes())
    }

    fn cd<T, K>(w: &mut Output<T, K>, args: &[&str]) -> anyhow::Result<()>
    where
        T: io::Write,
        K: io::Write,
    {
        if args.len() == 0 {
            return Ok(());
        }
        if args.len() > 1 {
            write_and_flush_str(&mut w.err, "cd: too many arguments")?;
            return Ok(());
        }

        let dir = Self::replace_with_home_dir(&args[0]);
        if env::set_current_dir(&dir).is_err() {
            write_and_flush_str(
                &mut w.out,
                &format!("cd: {}: No such file or directory", dir),
            )?;
        }
        Ok(())
    }

    fn exec<T, K>(
        w: &mut Output<T, K>,
        name: &str,
        path: PathBuf,
        args: &[&str],
    ) -> anyhow::Result<()>
    where
        T: io::Write,
        K: io::Write,
    {
        let output = process::Command::new(name)
            .args(args)
            .output()
            .context(format!(
                "failed to execute program {name} ({})",
                path.display()
            ))?;

        w.out
            .write_all(&output.stdout)
            .context("failed to write program output")?;
        w.err
            .write_all(&output.stderr)
            .context("failed to write program errors")?;
        Ok(())
    }

    fn command_not_found<T: io::Write>(w: &mut T, command: &str) -> anyhow::Result<()> {
        write_and_flush_str(w, &format!("{command}: command not found"))
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
