//! Каталог данных (ADR-0010).

use std::path::Path;

/// Каталог данных создаётся с режимом 0700 (как `mcp/`, ADR-0010): при umask 002 каталог с режимом 0775 считался бы
/// небезопасным, и ключи доступа ИИ-агентов из файла настроек не применялись бы никогда. Существующий каталог не меняется.
pub(crate) fn create(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}
