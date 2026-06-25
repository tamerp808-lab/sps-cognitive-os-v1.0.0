//! Embedding generation interface.
//!
//! The kernel is embedding-model-agnostic. The caller supplies an
//! `EmbeddingFn` that knows how to call the configured provider's
//! embedding endpoint (or compute a local hash-based embedding for tests).

use std::sync::Arc;

/// A function that generates an embedding for a piece of text.
/// Returns a vector of f32 values.
pub trait EmbeddingFn: Send + Sync {
    /// Generate an embedding for the given text.
    fn embed(&self, text: &str) -> Result<Vec<f32>, anyhow::Error>;

    /// Dimension of the embedding.
    fn dimension(&self) -> usize;
}

/// Embedding generator — wraps an `EmbeddingFn` and provides batch
/// helpers.
pub struct EmbeddingGenerator {
    fun: Arc<dyn EmbeddingFn>,
}

impl EmbeddingGenerator {
    /// Create a new generator.
    pub fn new(fun: Arc<dyn EmbeddingFn>) -> Self {
        Self { fun }
    }

    /// Embed a single text.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, anyhow::Error> {
        self.fun.embed(text)
    }

    /// Embed multiple texts.
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, anyhow::Error> {
        texts.iter().map(|t| self.fun.embed(t)).collect()
    }

    /// Dimension of the embeddings.
    pub fn dimension(&self) -> usize {
        self.fun.dimension()
    }
}

/// A hash-based embedding function for tests. Produces a deterministic
/// fixed-dimension vector from text via FNV-1a hashing into buckets.
/// Not semantically meaningful but is deterministic and fast.
pub struct HashEmbedding {
    dimension: usize,
}

impl HashEmbedding {
    /// Create a new hash embedding with the given dimension.
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

impl EmbeddingFn for HashEmbedding {
    fn embed(&self, text: &str) -> Result<Vec<f32>, anyhow::Error> {
        let mut vector = vec![0.0_f32; self.dimension];
        // FNV-1a hash each word and accumulate into buckets.
        for word in text.split_whitespace() {
            let mut hash = 2166136261u32;
            for b in word.bytes() {
                hash ^= b as u32;
                hash = hash.wrapping_mul(16777619);
            }
            let bucket = (hash as usize) % self.dimension;
            vector[bucket] += 1.0;
        }
        // Normalize to unit length.
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vector {
                *v /= norm;
            }
        }
        Ok(vector)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

/// Convenience: hash-based embedding constructor.
pub fn hash_embedding(dimension: usize) -> Arc<dyn EmbeddingFn> {
    Arc::new(HashEmbedding::new(dimension))
}
