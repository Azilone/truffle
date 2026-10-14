//! The EmbeddingGemma 2 files Truffle runs (ggml-org's Q8_0 GGUF at one pinned revision, as DigUp does: the F16
//! files overflow), and the prompts the model card asks for.

use std::path::{Path, PathBuf};

pub const REPOSITORY: &str = "ggml-org/embeddinggemma-2-GGUF";
pub const REVISION: &str = "bfcd298762cc34d0357ece5ebdd31791a3a374d8";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelFile {
    pub name: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

impl ModelFile {
    /// Hugging Face's address for this file at the pinned revision.
    pub fn url(&self) -> String {
        format!("https://huggingface.co/{REPOSITORY}/resolve/{REVISION}/{}", self.name)
    }
}

/// The text model (310 MB): all that queries need.
pub const TEXT: ModelFile = ModelFile {
    name: "embeddinggemma-2-Q8_0.gguf",
    bytes: 309_855_456,
    sha256: "2188ac1deca4b77dffefd603c2776a9d76d9d74ec01841392982ebb840b09135",
};

/// The image and audio encoders (555 MB).
pub const MEDIA: ModelFile = ModelFile {
    name: "mmproj-embeddinggemma-2-Q8_0.gguf",
    bytes: 554_821_024,
    sha256: "c4a8a52691ecef40618438928bdf9e68379b854e24166f292592353db0aab64f",
};

pub const ALL: [ModelFile; 2] = [TEXT, MEDIA];

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// Truffle's own models folder: beside the index.
pub fn models_dir() -> PathBuf {
    let home = home().unwrap_or_else(|| PathBuf::from("/"));
    #[cfg(target_os = "macos")]
    let base = home.join("Library/Application Support/Truffle");
    #[cfg(not(target_os = "macos"))]
    let base = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()).map_or_else(|| home.join(".local/share"), PathBuf::from).join("truffle");
    base.join("models")
}

/// The pinned revision's folder in the Hugging Face cache.
pub fn hugging_face_snapshot() -> Option<PathBuf> {
    let hub = std::env::var_os("HF_HUB_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HF_HOME").map(|h| PathBuf::from(h).join("hub")))
        .or_else(|| home().map(|h| h.join(".cache/huggingface/hub")))?;
    Some(hub.join(format!("models--{}", REPOSITORY.replace('/', "--"))).join("snapshots").join(REVISION))
}

/// DigUp's models folder: the same files, so a Mac that has DigUp needs no second download.
pub fn digup_models() -> Option<PathBuf> {
    cfg!(target_os = "macos").then(|| home().map(|h| h.join("Library/Application Support/DigUp/Models"))).flatten()
}

/// The files not in `folder` yet.
pub fn missing(folder: &Path) -> Vec<ModelFile> {
    ALL.into_iter().filter(|f| !folder.join(f.name).is_file()).collect()
}

pub struct Prompts;

impl Prompts {
    pub fn query(text: &str) -> String {
        format!("task: search result | query: {text}")
    }

    /// A search of the code index: the model card's prompt for finding code by what it does.
    pub fn code_query(text: &str) -> String {
        format!("task: code retrieval | query: {text}")
    }

    pub fn document(title: Option<&str>, text: &str) -> String {
        let title = title.filter(|t| !t.is_empty()).unwrap_or("none");
        format!("title: {title} | text: {text}")
    }
}
