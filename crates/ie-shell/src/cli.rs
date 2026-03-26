use anyhow::{Context, Result};
use clap::Parser;
use url::Url;

#[derive(Parser)]
#[command(
    name = "internet-exploder",
    about = "A minimal, private-by-default web browser"
)]
pub struct Cli {
    /// Run without a window
    #[arg(long)]
    pub headless: bool,

    /// URL to navigate to on startup
    #[arg(long)]
    pub url: Option<String>,

    /// Print page source to stdout and exit (requires --headless and --url)
    #[arg(long, requires_all = ["headless", "url"], conflicts_with = "dump_status")]
    pub dump_source: bool,

    /// Print HTTP status code to stdout and exit (requires --headless and --url)
    #[arg(long, requires_all = ["headless", "url"], conflicts_with = "dump_source")]
    pub dump_status: bool,

    /// Allow plain HTTP navigation (default: HTTPS-only)
    #[arg(long)]
    pub allow_http: bool,

    /// Override data directory (bookmarks, etc.)
    #[arg(long)]
    pub data_dir: Option<String>,

    /// Internal: subprocess kind (not shown in help)
    #[arg(long, hide = true)]
    pub subprocess_kind: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Mode {
    Gui {
        url: Option<Url>,
    },
    Headless {
        url: Option<Url>,
        action: HeadlessAction,
    },
    Subprocess {
        kind: ie_sandbox::ProcessKind,
    },
}

#[derive(Debug, PartialEq)]
pub enum HeadlessAction {
    DumpSource,
    DumpStatus,
    Interactive,
}

impl Cli {
    pub fn mode(&self) -> Result<Mode> {
        if let Some(kind_str) = &self.subprocess_kind {
            let kind = ie_sandbox::ProcessKind::parse(kind_str)
                .ok_or_else(|| anyhow::anyhow!("invalid subprocess kind: {kind_str}"))?;
            return Ok(Mode::Subprocess { kind });
        }

        let url = self
            .url
            .as_deref()
            .map(|s| Url::parse(s).with_context(|| format!("invalid URL: {s}")))
            .transpose()?;

        if self.headless {
            let action = if self.dump_source {
                HeadlessAction::DumpSource
            } else if self.dump_status {
                HeadlessAction::DumpStatus
            } else {
                HeadlessAction::Interactive
            };
            Ok(Mode::Headless { url, action })
        } else {
            Ok(Mode::Gui { url })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        let mut full = vec!["ie"];
        full.extend(args);
        Cli::try_parse_from(full).unwrap()
    }

    fn try_parse(args: &[&str]) -> Result<Cli, clap::Error> {
        let mut full = vec!["ie"];
        full.extend(args);
        Cli::try_parse_from(full)
    }

    #[test]
    fn no_args_gui_mode() {
        let cli = parse(&[]);
        let mode = cli.mode().unwrap();
        assert_eq!(mode, Mode::Gui { url: None });
    }

    #[test]
    fn url_gui_mode() {
        let cli = parse(&["--url", "https://example.com"]);
        let mode = cli.mode().unwrap();
        assert_eq!(
            mode,
            Mode::Gui {
                url: Some(Url::parse("https://example.com").unwrap())
            }
        );
    }

    #[test]
    fn headless_interactive() {
        let cli = parse(&["--headless"]);
        let mode = cli.mode().unwrap();
        assert_eq!(
            mode,
            Mode::Headless {
                url: None,
                action: HeadlessAction::Interactive
            }
        );
    }

    #[test]
    fn headless_with_url() {
        let cli = parse(&["--headless", "--url", "https://example.com"]);
        let mode = cli.mode().unwrap();
        assert_eq!(
            mode,
            Mode::Headless {
                url: Some(Url::parse("https://example.com").unwrap()),
                action: HeadlessAction::Interactive
            }
        );
    }

    #[test]
    fn headless_dump_source() {
        let cli = parse(&[
            "--headless",
            "--dump-source",
            "--url",
            "https://example.com",
        ]);
        let mode = cli.mode().unwrap();
        assert_eq!(
            mode,
            Mode::Headless {
                url: Some(Url::parse("https://example.com").unwrap()),
                action: HeadlessAction::DumpSource
            }
        );
    }

    #[test]
    fn headless_dump_status() {
        let cli = parse(&[
            "--headless",
            "--dump-status",
            "--url",
            "https://example.com",
        ]);
        let mode = cli.mode().unwrap();
        assert_eq!(
            mode,
            Mode::Headless {
                url: Some(Url::parse("https://example.com").unwrap()),
                action: HeadlessAction::DumpStatus
            }
        );
    }

    #[test]
    fn dump_source_without_headless_fails() {
        assert!(try_parse(&["--dump-source", "--url", "https://example.com"]).is_err());
    }

    #[test]
    fn dump_source_without_url_fails() {
        assert!(try_parse(&["--headless", "--dump-source"]).is_err());
    }

    #[test]
    fn dump_source_and_dump_status_conflict() {
        assert!(
            try_parse(&[
                "--headless",
                "--dump-source",
                "--dump-status",
                "--url",
                "https://example.com"
            ])
            .is_err()
        );
    }

    #[test]
    fn allow_http_flag() {
        let cli = parse(&["--allow-http"]);
        assert!(cli.allow_http);
    }

    #[test]
    fn invalid_url_returns_error() {
        let cli = parse(&["--url", "not://[a valid url"]);
        assert!(cli.mode().is_err());
    }
}
