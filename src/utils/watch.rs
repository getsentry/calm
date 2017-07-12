use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

use notify::{Watcher, RecursiveMode, DebouncedEvent, watcher};

use prelude::*;

pub fn watch_files(path: &Path, cb: &Fn(&Path) -> Result<()>) -> Result<()> {
    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_millis(100)).unwrap();

    watcher.watch(path, RecursiveMode::Recursive).unwrap();

    loop {
        match rx.recv() {
            Ok(DebouncedEvent::Create(path)) => { cb(&path)? }
            Ok(DebouncedEvent::Write(path)) => { cb(&path)? }
            Ok(DebouncedEvent::Rename(_, path)) => { cb(&path)? }
            Ok(..) => {}
            Err(err) => {
                panic!("Failed to watch: {}", err);
            }
        }
    }
}
