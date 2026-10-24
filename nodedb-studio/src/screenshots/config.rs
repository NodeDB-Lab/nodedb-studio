//! Command-line switch that turns a `screenshots` build into a scene run.

use std::path::PathBuf;

/// Where a scene run writes its PNGs. Provided as a root context: its
/// presence is what starts the scene runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunConfig {
    pub out_dir: PathBuf,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("`--screenshots` needs an output directory: `--screenshots <dir>`")]
    MissingDir,
    #[error("unknown argument `{0}`: the only argument is `--screenshots <dir>`")]
    Unknown(String),
}

impl RunConfig {
    /// Parse the arguments after the program name. No arguments is an
    /// interactive session (`Ok(None)`).
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Option<Self>, ConfigError> {
        let mut args = args.into_iter();
        let Some(flag) = args.next() else {
            return Ok(None);
        };
        if flag != "--screenshots" {
            return Err(ConfigError::Unknown(flag));
        }
        let dir = args.next().ok_or(ConfigError::MissingDir)?;
        if let Some(extra) = args.next() {
            return Err(ConfigError::Unknown(extra));
        }
        Ok(Some(Self {
            out_dir: PathBuf::from(dir),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<RunConfig>, ConfigError> {
        RunConfig::from_args(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn no_arguments_is_an_interactive_session() {
        assert_eq!(parse(&[]), Ok(None));
    }

    #[test]
    fn screenshots_flag_takes_the_output_directory() {
        let run = parse(&["--screenshots", "out"]).expect("valid arguments");
        assert_eq!(run.map(|r| r.out_dir), Some(PathBuf::from("out")));
    }

    #[test]
    fn missing_directory_is_an_error() {
        assert_eq!(parse(&["--screenshots"]), Err(ConfigError::MissingDir));
    }

    #[test]
    fn unknown_and_extra_arguments_are_errors() {
        assert_eq!(
            parse(&["--shots", "out"]),
            Err(ConfigError::Unknown("--shots".into()))
        );
        assert_eq!(
            parse(&["--screenshots", "out", "more"]),
            Err(ConfigError::Unknown("more".into()))
        );
    }
}
