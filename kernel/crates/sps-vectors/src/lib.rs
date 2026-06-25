//! SPS Vector Search (Phase 3.5).
//!
//! Provides an in-memory vector index for semantic search over memories.
//! Uses cosine similarity and a simple brute-force scan (suitable for
//! thousands of vectors; for millions, swap in `hnswlib` or `usearch`).
//!
//! Embeddings are generated via the Effect Manager's `llm.complete`
//! effect — the caller supplies an embedding function, this crate
//! handles storage and search.

#![allow(clippy::module_name_repetitions)]

pub mod index;
pub mod embedding;
pub mod reducer;

pub use index::{VectorIndex, VectorEntry, SearchResult, SimilarityMetric};
pub use embedding::{EmbeddingFn, EmbeddingGenerator, hash_embedding};
pub use reducer::{VectorReducer, VectorState};
