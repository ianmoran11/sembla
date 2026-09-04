use super::*;

#[test]
fn publication_replaces_whole_files_without_leaving_temporary_paths() {
    let directory = std::env::temp_dir().join(format!(
        "sembla-atomic-write-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("result.csv");

    write_atomic(&path, b"first\n").unwrap();
    write_atomic(&path, b"second\n").unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), b"second\n");
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    std::fs::remove_dir_all(directory).unwrap();
}
