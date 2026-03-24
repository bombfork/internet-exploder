use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessKind {
    Browser,
    Renderer,
    Network,
}

pub fn spawn_child(_kind: ProcessKind) -> Result<()> {
    todo!("Spawn sandboxed child process")
}
