//! The command line of `kinige-server`: no arguments (or `serve`) runs the server, `backup <dir> [--prune]` writes a backup, `restore
//! <backup directory>` loads one. Parsed by hand: three commands do not need a framework.

use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Serve,
    Backup { dir: PathBuf, prune: bool },
    Restore { dir: PathBuf },
}

pub const USAGE: &str =
    "usage: kinige-server [serve] | backup <dir> [--prune] | restore <backup directory>";

/// `args` without the program name.
pub fn parse(args: &[String]) -> Result<Command, String> {
    let mut args = args.iter().map(String::as_str);
    match args.next() {
        None | Some("serve") => no_more(args, Command::Serve),
        Some("backup") => {
            let dir = args.next().ok_or(USAGE)?;
            let prune = match args.next() {
                None => false,
                Some("--prune") => true,
                Some(_) => return Err(USAGE.to_string()),
            };
            no_more(
                args,
                Command::Backup {
                    dir: PathBuf::from(dir),
                    prune,
                },
            )
        }
        Some("restore") => {
            let dir = args.next().ok_or(USAGE)?;
            no_more(
                args,
                Command::Restore {
                    dir: PathBuf::from(dir),
                },
            )
        }
        Some(_) => Err(USAGE.to_string()),
    }
}

fn no_more<'a>(
    mut rest: impl Iterator<Item = &'a str>,
    command: Command,
) -> Result<Command, String> {
    if rest.next().is_some() {
        Err(USAGE.to_string())
    } else {
        Ok(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn the_three_commands_and_nothing_else() {
        assert_eq!(parse(&args(&[])), Ok(Command::Serve));
        assert_eq!(parse(&args(&["serve"])), Ok(Command::Serve));
        assert_eq!(
            parse(&args(&["backup", "/b", "--prune"])),
            Ok(Command::Backup {
                dir: PathBuf::from("/b"),
                prune: true
            })
        );
        assert_eq!(
            parse(&args(&["backup", "/b"])),
            Ok(Command::Backup {
                dir: PathBuf::from("/b"),
                prune: false
            })
        );
        assert_eq!(
            parse(&args(&["restore", "/b/backup-1"])),
            Ok(Command::Restore {
                dir: PathBuf::from("/b/backup-1")
            })
        );
        for bad in [
            vec!["backup"],
            vec!["backup", "/b", "--prune", "x"],
            vec!["backup", "/b", "--force"],
            vec!["restore"],
            vec!["restore", "a", "b"],
            vec!["serve", "x"],
            vec!["frobnicate"],
            vec![""],
        ] {
            assert!(parse(&args(&bad)).is_err(), "{bad:?}");
        }
    }
}
