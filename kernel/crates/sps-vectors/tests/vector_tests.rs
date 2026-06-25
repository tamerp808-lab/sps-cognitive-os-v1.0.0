//! Phase 3.5 — Vector search tests.

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_vectors::embedding::{hash_embedding, EmbeddingFn, EmbeddingGenerator};
use sps_vectors::index::{similarity, SimilarityMetric, VectorEntry, VectorIndex};
use sps_vectors::reducer::{VectorReducer, VectorState};
use uuid::Uuid;

#[test]
fn hash_embedding_is_deterministic_and_normalized() {
    let emb = hash_embedding(64);
    let v1 = emb.embed("hello world").unwrap();
    let v2 = emb.embed("hello world").unwrap();
    assert_eq!(v1, v2);
    assert_eq!(v1.len(), 64);
    // Unit norm.
    let norm: f32 = v1.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 0.001);
}

#[test]
fn hash_embedding_different_text_produces_different_vectors() {
    let emb = hash_embedding(64);
    let v1 = emb.embed("rust programming").unwrap();
    let v2 = emb.embed("python programming").unwrap();
    assert_ne!(v1, v2);
}

#[test]
fn vector_index_add_and_get() {
    let index = VectorIndex::new();
    let entry = VectorEntry {
        id: Uuid::now_v7(),
        vector: vec![1.0, 0.0, 0.0],
        text: Some("test".into()),
        metadata: json!({}),
    };
    let id = entry.id;
    index.add(entry).unwrap();
    assert_eq!(index.len(), 1);
    assert!(index.get(&id).is_some());
    assert_eq!(index.dimension(), 3);
}

#[test]
fn vector_index_rejects_dimension_mismatch() {
    let index = VectorIndex::new();
    index.add(VectorEntry {
        id: Uuid::now_v7(),
        vector: vec![1.0, 0.0],
        text: None,
        metadata: json!({}),
    }).unwrap();
    let result = index.add(VectorEntry {
        id: Uuid::now_v7(),
        vector: vec![1.0, 0.0, 0.0],
        text: None,
        metadata: json!({}),
    });
    assert!(result.is_err());
}

#[test]
fn vector_index_search_cosine() {
    let index = VectorIndex::new();
    index.add(VectorEntry {
        id: Uuid::now_v7(),
        vector: vec![1.0, 0.0, 0.0],
        text: Some("x-axis".into()),
        metadata: json!({}),
    }).unwrap();
    index.add(VectorEntry {
        id: Uuid::now_v7(),
        vector: vec![0.0, 1.0, 0.0],
        text: Some("y-axis".into()),
        metadata: json!({}),
    }).unwrap();
    index.add(VectorEntry {
        id: Uuid::now_v7(),
        vector: vec![0.7, 0.7, 0.0],
        text: Some("diagonal".into()),
        metadata: json!({}),
    }).unwrap();

    let results = index.search(&[1.0, 0.0, 0.0], 3);
    assert_eq!(results.len(), 3);
    // The x-axis entry should be the most similar to [1,0,0].
    assert!((results[0].score - 1.0).abs() < 0.001);
}

#[test]
fn vector_index_search_dot_product() {
    let index = VectorIndex::new();
    index.set_metric(SimilarityMetric::Dot);
    index.add(VectorEntry {
        id: Uuid::now_v7(),
        vector: vec![2.0, 3.0],
        text: None,
        metadata: json!({}),
    }).unwrap();
    index.add(VectorEntry {
        id: Uuid::now_v7(),
        vector: vec![1.0, 1.0],
        text: None,
        metadata: json!({}),
    }).unwrap();
    let results = index.search(&[1.0, 1.0], 2);
    // dot([1,1],[2,3]) = 5, dot([1,1],[1,1]) = 2.
    assert!((results[0].score - 5.0).abs() < 0.001);
    assert!((results[1].score - 2.0).abs() < 0.001);
}

#[test]
fn vector_index_remove() {
    let index = VectorIndex::new();
    let id = Uuid::now_v7();
    index.add(VectorEntry {
        id,
        vector: vec![1.0],
        text: None,
        metadata: json!({}),
    }).unwrap();
    assert_eq!(index.len(), 1);
    assert!(index.remove(&id).is_some());
    assert_eq!(index.len(), 0);
}

#[test]
fn similarity_handles_zero_vectors() {
    let s = similarity(&[0.0, 0.0], &[1.0, 0.0], SimilarityMetric::Cosine);
    assert_eq!(s, 0.0);
}

#[test]
fn similarity_handles_dimension_mismatch() {
    let s = similarity(&[1.0], &[1.0, 0.0], SimilarityMetric::Cosine);
    assert_eq!(s, 0.0);
}

#[test]
fn embedding_generator_batch() {
    let gen = EmbeddingGenerator::new(hash_embedding(32));
    let texts = vec!["hello".to_string(), "world".to_string()];
    let vectors = gen.embed_batch(&texts).unwrap();
    assert_eq!(vectors.len(), 2);
    assert_eq!(vectors[0].len(), 32);
}

#[test]
fn vector_added_event_persists_entry() {
    let pipeline = {
        let mut reg = ReducerRegistry::new();
        VectorReducer::register(&mut reg);
        Arc::new(ReducerPipeline::new(Arc::new(reg)))
    };
    let mut state = CanonicalState::genesis();
    let entry = VectorEntry {
        id: Uuid::now_v7(),
        vector: vec![1.0, 0.0, 0.0],
        text: Some("test".into()),
        metadata: json!({}),
    };
    let event = RawEvent::new(
        "vector.added",
        serde_json::to_value(&entry).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let vs = VectorState::from_state(&state).unwrap();
    assert_eq!(vs.entries.len(), 1);
}

#[test]
fn vector_state_rebuilds_index() {
    let mut vs = VectorState::default();
    vs.entries.insert(
        Uuid::now_v7(),
        VectorEntry {
            id: Uuid::now_v7(),
            vector: vec![1.0, 0.0],
            text: None,
            metadata: json!({}),
        },
    );
    let index = vs.to_index();
    assert_eq!(index.len(), 1);
    assert_eq!(index.dimension(), 2);
}

#[test]
fn end_to_end_semantic_search_with_hash_embedding() {
    // Build an index with multiple texts and search.
    let emb = hash_embedding(128);
    let gen = EmbeddingGenerator::new(emb);
    let index = VectorIndex::new();

    let texts = vec![
        "rust is a systems programming language",
        "python is a dynamic scripting language",
        "rust has ownership and borrowing",
        "machine learning with python",
        "the rust compiler is fast",
    ];
    for text in &texts {
        let vector = gen.embed(text).unwrap();
        index.add(VectorEntry {
            id: Uuid::now_v7(),
            vector,
            text: Some(text.to_string()),
            metadata: json!({}),
        }).unwrap();
    }

    // Search for "rust programming".
    let query = gen.embed("rust programming").unwrap();
    let results = index.search(&query, 5);
    assert_eq!(results.len(), 5);

    // The top results should contain "rust" in the text.
    let top_ids: Vec<Uuid> = results.iter().map(|r| r.id).collect();
    let mut rust_count = 0;
    for id in &top_ids {
        if let Some(entry) = index.get(id) {
            if entry
                .text
                .as_ref()
                .map(|t| t.contains("rust"))
                .unwrap_or(false)
            {
                rust_count += 1;
            }
        }
    }
    // At least 2 of the top 3 should mention rust.
    assert!(
        rust_count >= 2,
        "expected at least 2 rust-related results in top 5, got {}",
        rust_count
    );
}
