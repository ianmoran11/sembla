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

#[test]
fn concurrent_publications_never_share_a_temporary_path() {
    let directory = std::env::temp_dir().join(format!(
        "sembla-atomic-write-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("concurrent")
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = std::sync::Arc::new(directory.join("result.csv"));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let payloads = (0..8)
        .map(|index| format!("writer-{index}\n").into_bytes())
        .collect::<Vec<_>>();

    let threads = payloads
        .iter()
        .cloned()
        .map(|payload| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                write_atomic(path.as_ref(), &payload)
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap().unwrap();
    }

    let published = std::fs::read(path.as_ref()).unwrap();
    assert!(payloads.contains(&published));
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    std::fs::remove_dir_all(directory).unwrap();
}
