//! trovara read Windows' NTFS change journal to catch up after a restart. macOS and Linux have none the indexer can
//! read without privileges: a start syncs the folders instead (and the daemon's watcher feeds changes while it runs).

use std::path::Path;

use truffle_semantic::IndexStore;

pub fn installed() -> bool {
    false
}

pub fn volume_of(_path: &str) -> Option<String> {
    None
}

pub struct VolumeAnswer {
    pub volume: String,
    pub folders: Option<Vec<String>>,
}

pub fn catch_up(_index_folder: &Path, _store: &IndexStore, _roots: &[String]) -> Option<Vec<VolumeAnswer>> {
    None
}

pub fn checkpoint(_index_folder: &Path, _store: &IndexStore, _roots: &[String]) -> bool {
    false
}
