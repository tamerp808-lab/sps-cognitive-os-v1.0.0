//! Vector index — stores embeddings and supports similarity search.

use std::collections::BTreeMap;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Similarity metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimilarityMetric {
    /// Cosine similarity (1 - cosine distance).
    Cosine,
    /// Euclidean distance (negated, so higher = more similar).
    Euclidean,
    /// Dot product.
    Dot,
}

impl Default for SimilarityMetric {
    fn default() -> Self {
        Self::Cosine
    }
}

/// A vector entry — an embedding + metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorEntry {
    /// Unique id (typically the memory id).
    pub id: Uuid,
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// Optional text that was embedded (for debugging / display).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Optional metadata.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// A search result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    /// The matched entry's id.
    pub id: Uuid,
    /// Similarity score (higher = more similar; interpretation depends on metric).
    pub score: f32,
}

/// The vector index — thread-safe, in-memory.
pub struct VectorIndex {
    entries: RwLock<BTreeMap<Uuid, VectorEntry>>,
    dimension: RwLock<usize>,
    metric: RwLock<SimilarityMetric>,
}

impl Default for VectorIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorIndex {
    /// Create a new empty index with cosine similarity.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(BTreeMap::new()),
            dimension: RwLock::new(0),
            metric: RwLock::new(SimilarityMetric::Cosine),
        }
    }

    /// Set the similarity metric.
    pub fn set_metric(&self, metric: SimilarityMetric) {
        *self.metric.write() = metric;
    }

    /// Add an entry. If the dimension is unset, it is set from the first
    /// entry. Subsequent entries must match the dimension.
    pub fn add(&self, entry: VectorEntry) -> Result<(), String> {
        let mut dim = self.dimension.write();
        if *dim == 0 {
            *dim = entry.vector.len();
        } else if entry.vector.len() != *dim {
            return Err(format!(
                "dimension mismatch: expected {}, got {}",
                *dim,
                entry.vector.len()
            ));
        }
        drop(dim);
        self.entries.write().insert(entry.id, entry);
        Ok(())
    }

    /// Remove an entry.
    pub fn remove(&self, id: &Uuid) -> Option<VectorEntry> {
        self.entries.write().remove(id)
    }

    /// Get an entry by id.
    pub fn get(&self, id: &Uuid) -> Option<VectorEntry> {
        self.entries.read().get(id).cloned()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Is the index empty?
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    /// Vector dimension.
    pub fn dimension(&self) -> usize {
        *self.dimension.read()
    }

    /// Search for the top-k most similar entries to the query vector.
    pub fn search(&self, query: &[f32], limit: usize) -> Vec<SearchResult> {
        let entries = self.entries.read();
        let metric = *self.metric.read();
        let mut scored: Vec<SearchResult> = entries
            .values()
            .map(|e| SearchResult {
                id: e.id,
                score: similarity(query, &e.vector, metric),
            })
            .collect();
        // Sort descending by score.
        scored.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);
        scored
    }

    /// Clear all entries.
    pub fn clear(&self) {
        self.entries.write().clear();
        *self.dimension.write() = 0;
    }
}

/// Compute similarity between two vectors.
pub fn similarity(a: &[f32], b: &[f32], metric: SimilarityMetric) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    match metric {
        SimilarityMetric::Cosine => {
            let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
            let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
            let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm_a == 0.0 || norm_b == 0.0 {
                return 0.0;
            }
            dot / (norm_a * norm_b)
        }
        SimilarityMetric::Euclidean => {
            let dist: f32 = a
                .iter()
                .zip(b)
                .map(|(x, y)| (x - y).powi(2))
                .sum::<f32>()
                .sqrt();
            -dist // negate so higher = more similar
        }
        SimilarityMetric::Dot => a.iter().zip(b).map(|(x, y)| x * y).sum(),
    }
}
