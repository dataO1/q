use anyhow::{anyhow, Result};
use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer_opt, DebouncedEvent, Debouncer, FileIdMap};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

pub enum FileEvent {
    Modified(PathBuf),
    Created(PathBuf),
    Removed(PathBuf),
}

pub struct FileWatcher {
    _debouncer: Debouncer<notify::RecommendedWatcher, FileIdMap>,
}

impl FileWatcher {
    pub fn new(
        paths: Vec<PathBuf>,
        debounce_duration: Duration,
    ) -> Result<(Self, mpsc::Receiver<FileEvent>)> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        let tx_clone = tx.clone();
        let mut debouncer = new_debouncer_opt(
            debounce_duration,
            None,
            move |result: Result<Vec<DebouncedEvent>, Vec<notify::Error>>| {
                match result {
                    Ok(events) => {
                        for event in events {
                            let file_event = match event.kind {
                                EventKind::Create(_) => {
                                    FileEvent::Created(event.paths[0].clone())
                                }
                                EventKind::Modify(_) => {
                                    FileEvent::Modified(event.paths[0].clone())
                                }
                                EventKind::Remove(_) => {
                                    FileEvent::Removed(event.paths[0].clone())
                                }
                                _ => continue,
                            };
                            let _ = tx_clone.send(file_event);
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            tracing::error!("Watch error: {:?}", error);
                        }
                    }
                }
            }, FileIdMap::new(),notify::Config::default()
        );
        let mut debouncer = debouncer?;

        for path in paths {
            match debouncer.watch(&path, RecursiveMode::Recursive) {
                Ok(_) => tracing::info!("Watching path: {:?}", path),
                Err(e) => tracing::warn!("Could not watch {:?}: {}", path, e),
            }
        }
        Ok((Self { _debouncer: debouncer }, rx))
    }
}
