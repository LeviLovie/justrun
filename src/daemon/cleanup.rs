use justrun::paths::TMP_PATHS;

pub fn cleanup() -> bool {
    let mut success = true;
    for path in TMP_PATHS {
        if std::path::Path::new(path).exists() {
            std::fs::remove_file(path).unwrap_or_else(|err| {
                eprintln!(
                    "Failed to remove {} during cleanup, consider deleting it manually: {}",
                    path, err
                );
                success = false;
            });
        }
    }
    return success;
}
