//! Data directory (ADR-0010).

use std::path::Path;

/// The data directory is created with mode 0700 (like `mcp/`, ADR-0010): it holds the database, the attachments and the MCP
/// socket's directory, and nobody else on the machine needs to read it. An existing directory is not changed.
pub(crate) fn create(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}
