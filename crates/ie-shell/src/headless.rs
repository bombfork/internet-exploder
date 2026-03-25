use anyhow::Result;
use url::Url;

use crate::cli::HeadlessAction;

pub fn run_headless(url: Option<Url>, action: HeadlessAction, _allow_http: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match action {
            HeadlessAction::DumpSource => {
                tracing::info!(url = ?url, "dump-source mode");
                println!("source dump not yet wired (need ie-net integration)");
            }
            HeadlessAction::DumpStatus => {
                tracing::info!(url = ?url, "dump-status mode");
                println!("status dump not yet wired (need ie-net integration)");
            }
            HeadlessAction::Interactive => {
                tracing::info!("interactive headless mode");
                eprintln!("interactive headless mode not yet implemented");
            }
        }
        Ok(())
    })
}
