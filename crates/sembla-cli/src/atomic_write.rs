//! Same-directory atomic publication for CLI text artifacts.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn write_atomic(path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), String> {
    let path = path.as_ref();
    let name = path
        .file_name()
        .ok_or_else(|| format!("{}: output path has no file name", path.display()))?
        .to_string_lossy();
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_file_name(format!(
        ".{name}.sembla-tmp-{}-{sequence}",
        std::process::id()
    ));
    let result = (|| -> std::io::Result<()> {
        std::fs::write(&temporary, bytes)?;
        std::fs::File::open(&temporary)?.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(temporary);
        return Err(format!("{}: {error}", path.display()));
    }
    Ok(())
}

#[cfg(test)]
#[path = "atomic_write_tests.rs"]
mod tests;
