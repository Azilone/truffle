//! EmbeddingGemma 2 on llama.cpp: the FFI to llama.h and mtmd.h, and the embedder that turns text, images and audio
//! into 768-dimensional L2-normalized vectors. Started from trovara-llama (MIT, Subhankar Patra).

mod embedder;
mod ffi;
pub mod image_sizing;
mod log;
pub mod model;
pub mod reference;

pub use embedder::{EmbedError, Embedder, Models, devices, threads, vector_source};
pub use model::Prompts;

/// Which vectors this pipeline makes. It changes only when the vectors do (checked by the reference-vector test), and
/// indexes record it: a new number means every index embeds again.
pub const VECTOR_VERSION: u32 = 1;

/// The llama.cpp release Truffle builds (build.rs pins its commit).
pub const LLAMA_TAG: &str = "b11461";

/// The vision soft-token budgets the model accepts.
pub const BUDGETS: [usize; 5] = [70, 140, 280, 560, 1120];

/// Each vector's cosine with its reference must stay at least this.
pub const AGREEMENT: f32 = 0.999;
