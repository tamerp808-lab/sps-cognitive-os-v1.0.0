//! Knowledge Graph Expander — auto-links related memories.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A discovered link between two memories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLink {
    pub from: Uuid,
    pub to: Uuid,
    pub link_type: LinkType,
    pub strength: f64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    /// Memories share common tags.
    SharedTags,
    /// One memory caused/led to another.
    Causal,
    /// Memories are about the same topic.
    SameTopic,
    /// One memory is a refinement of another.
    Refinement,
    /// Memories occurred in the same context.
    SameContext,
    /// One memory contradicts another.
    Contradiction,
}

/// A memory for graph expansion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMemory {
    pub id: Uuid,
    pub tags: Vec<String>,
    pub title: String,
    pub content_keywords: Vec<String>,
    pub context_id: Option<Uuid>,
    pub timestamp_ms: u64,
}

/// The Knowledge Graph Expander.
pub struct KnowledgeGraphExpander {
    /// Minimum tag overlap for SharedTags links.
    pub min_tag_overlap: usize,
    /// Minimum keyword overlap for SameTopic links.
    pub min_keyword_overlap: usize,
    /// Time window for SameContext links (ms).
    pub context_time_window_ms: u64,
}

impl Default for KnowledgeGraphExpander {
    fn default() -> Self {
        Self {
            min_tag_overlap: 1,
            min_keyword_overlap: 2,
            context_time_window_ms: 60_000,
        }
    }
}

impl KnowledgeGraphExpander {
    /// Find all links between a set of memories.
    pub fn expand(&self, memories: &[GraphMemory]) -> Vec<MemoryLink> {
        let mut links = Vec::new();

        for i in 0..memories.len() {
            for j in (i + 1)..memories.len() {
                let a = &memories[i];
                let b = &memories[j];

                // Check for shared tags.
                let shared_tags = a.tags.iter().filter(|t| b.tags.contains(t)).count();
                if shared_tags >= self.min_tag_overlap {
                    links.push(MemoryLink {
                        from: a.id,
                        to: b.id,
                        link_type: LinkType::SharedTags,
                        strength: (shared_tags as f64 / (a.tags.len().max(b.tags.len()) as f64)).min(1.0),
                        reason: format!("{} shared tags", shared_tags),
                    });
                }

                // Check for same topic (keyword overlap).
                let shared_keywords = a.content_keywords.iter()
                    .filter(|k| b.content_keywords.contains(k))
                    .count();
                if shared_keywords >= self.min_keyword_overlap {
                    links.push(MemoryLink {
                        from: a.id,
                        to: b.id,
                        link_type: LinkType::SameTopic,
                        strength: (shared_keywords as f64 / 10.0).min(1.0),
                        reason: format!("{} shared keywords", shared_keywords),
                    });
                }

                // Check for same context.
                if a.context_id.is_some() && a.context_id == b.context_id {
                    let time_diff = (a.timestamp_ms as i64 - b.timestamp_ms as i64).unsigned_abs();
                    if time_diff <= self.context_time_window_ms {
                        links.push(MemoryLink {
                            from: a.id,
                            to: b.id,
                            link_type: LinkType::SameContext,
                            strength: 1.0 - (time_diff as f64 / self.context_time_window_ms as f64),
                            reason: format!("same context, {}ms apart", time_diff),
                        });
                    }
                }
            }
        }

        links
    }

    /// Find the strongest links from a given memory.
    pub fn strongest_links_from<'a>(&self, links: &'a [MemoryLink], memory_id: Uuid, limit: usize) -> Vec<&'a MemoryLink> {
        let mut filtered: Vec<_> = links
            .iter()
            .filter(|l| l.from == memory_id || l.to == memory_id)
            .collect();
        filtered.sort_by(|a, b| {
            b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal)
        });
        filtered.truncate(limit);
        filtered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_memories_with_shared_tags() {
        let expander = KnowledgeGraphExpander::default();
        let memories = vec![
            GraphMemory {
                id: Uuid::nil(),
                tags: vec!["rust".into(), "api".into()],
                title: "REST API".into(),
                content_keywords: vec!["http".into(), "get".into(), "post".into()],
                context_id: None,
                timestamp_ms: 1000,
            },
            GraphMemory {
                id: Uuid::nil(),
                tags: vec!["rust".into(), "cli".into()],
                title: "CLI Tool".into(),
                content_keywords: vec!["http".into(), "post".into(), "command".into()],
                context_id: None,
                timestamp_ms: 2000,
            },
        ];
        let links = expander.expand(&memories);
        assert!(!links.is_empty());
        assert!(links.iter().any(|l| l.link_type == LinkType::SharedTags));
        assert!(links.iter().any(|l| l.link_type == LinkType::SameTopic));
    }

    #[test]
    fn links_same_context_memories() {
        let expander = KnowledgeGraphExpander::default();
        let ctx = Uuid::nil();
        let memories = vec![
            GraphMemory {
                id: Uuid::nil(),
                tags: vec![],
                title: "A".into(),
                content_keywords: vec![],
                context_id: Some(ctx),
                timestamp_ms: 1000,
            },
            GraphMemory {
                id: Uuid::nil(),
                tags: vec![],
                title: "B".into(),
                content_keywords: vec![],
                context_id: Some(ctx),
                timestamp_ms: 5000,
            },
        ];
        let links = expander.expand(&memories);
        assert!(links.iter().any(|l| l.link_type == LinkType::SameContext));
    }

    #[test]
    fn no_links_for_unrelated_memories() {
        let expander = KnowledgeGraphExpander::default();
        let memories = vec![
            GraphMemory {
                id: Uuid::nil(),
                tags: vec!["a".into()],
                title: "A".into(),
                content_keywords: vec!["x".into()],
                context_id: None,
                timestamp_ms: 1000,
            },
            GraphMemory {
                id: Uuid::nil(),
                tags: vec!["b".into()],
                title: "B".into(),
                content_keywords: vec!["y".into()],
                context_id: None,
                timestamp_ms: 2000,
            },
        ];
        let links = expander.expand(&memories);
        assert!(links.is_empty());
    }
}
