use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

pub struct FileWatcher {
    path: PathBuf,
    _watcher: notify::RecommendedWatcher,
    changed: Arc<AtomicBool>,
}

impl FileWatcher {
    pub fn new(path: PathBuf) -> Result<Self> {
        let parent = path.parent().expect("File has no parent");

        let changed = Arc::new(AtomicBool::new(false));
        let changed_in_watcher = changed.clone();

        let mut watcher =
            notify::recommended_watcher(move |maybe_event: notify::Result<notify::Event>| {
                match maybe_event {
                    Ok(event) => {
                        if event.kind.is_modify() || event.kind.is_create() {
                            changed_in_watcher.store(true, Ordering::Relaxed);
                        }
                    }
                    Err(e) => eprintln!("Watcher failed {:?}", e),
                }
            })?;

        watcher.watch(&parent, RecursiveMode::NonRecursive)?;

        Ok(FileWatcher {
            path,
            _watcher: watcher,
            changed,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn changed(&self) -> bool {
        self.changed.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.changed.store(false, Ordering::Relaxed);
    }
}
