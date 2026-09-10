//! Developer-only filesystem notification probe. Prints no paths or log contents.
use notify::Watcher;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let home = codexmeter_core::codex_home();
    let root = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or(home);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher =
        notify::recommended_watcher(move |event: Result<notify::Event, notify::Error>| {
            let _ = tx.send(event);
        })?;
    watcher.watch(&root, notify::RecursiveMode::Recursive)?;
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut count = 0;
    while let Ok(event) = rx.recv_timeout(end.saturating_duration_since(std::time::Instant::now()))
    {
        match event {
            Ok(e) => {
                count += 1;
                println!(
                    "kind={:?} paths={} matched_root={}",
                    e.kind,
                    e.paths.len(),
                    e.paths.iter().all(|p| p.starts_with(&root))
                );
            }
            Err(_) => println!("watcher_error"),
        };
        if std::time::Instant::now() >= end {
            break;
        }
    }
    println!("events={count}");
    Ok(())
}
