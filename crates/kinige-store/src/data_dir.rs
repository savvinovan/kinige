//! Data directory (ADR-0010).

use std::path::Path;

/// The data directory is created with mode 0700 (like `mcp/`, ADR-0010): with umask 002 a directory with mode 0775
/// would count as unsafe, and the AI agents' access keys from the settings file would never be applied. An existing
/// directory is not changed.
pub(crate) fn create(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}
