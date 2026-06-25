//! Search executor — in-memory text search (Phase 1 minimal; Phase 3 will
//! add vector search via sqlite-vec).

use crate::effect::{EffectError, EffectIntent, EffectResult, EffectType};
use crate::registry::EffectExecutor;
use serde::{Deserialize, Serialize};

/// Simple search executor.
pub struct SearchExecutor;

impl SearchExecutor {
    /// Create a new search executor.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SearchExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchInput {
    query: String,
    #[serde(default)]
    documents: Vec<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

impl EffectExecutor for SearchExecutor {
    fn name(&self) -> &'static str {
        "search"
    }

    fn execute(&self, intent: &EffectIntent, intent_tick: u64) -> Result<EffectResult, EffectError> {
        if intent.effect_type != EffectType::SearchQuery {
            return Err(EffectError::NoExecutor(intent.effect_type.as_str().to_string()));
        }
        let start = std::time::Instant::now();
        let input: SearchInput = serde_json::from_value(intent.input.clone()).map_err(|e| {
            EffectError::ExecutorFailed {
                message: format!("invalid search.query input: {}", e),
                details: None,
            }
        })?;

        let query_lower = input.query.to_lowercase();
        let mut hits: Vec<serde_json::Value> = input
            .documents
            .iter()
            .enumerate()
            .filter_map(|(i, doc)| {
                let doc_lower = doc.to_lowercase();
                if doc_lower.contains(&query_lower) {
                    Some(serde_json::json!({
                        "index": i,
                        "snippet": doc,
                        "score": 1.0_f64,
                    }))
                } else {
                    None
                }
            })
            .take(input.limit)
            .collect();

        // Sort by score (currently all 1.0 — real search in Phase 3).
        hits.sort_by(|a, b| {
            let sa = a["score"].as_f64().unwrap_or(0.0);
            let sb = b["score"].as_f64().unwrap_or(0.0);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(EffectResult {
            intent_tick,
            output: serde_json::json!({
                "query": input.query,
                "hits": hits,
                "total": hits.len(),
            }),
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}
