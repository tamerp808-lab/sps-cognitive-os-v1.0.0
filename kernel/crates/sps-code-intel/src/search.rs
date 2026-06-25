//! Fuzzy search — subsequence matching with scoring.
//!
//! Implements a simple but effective fuzzy search:
//! - Subsequence matching (chars of query appear in order in the target)
//! - Bonus for consecutive matches
//! - Bonus for matches at word boundaries
//! - Case-insensitive

use serde::{Deserialize, Serialize};

/// A search query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// The search string.
    pub query: String,
    /// Maximum results.
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Filter by symbol kind (None = all).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<crate::symbol::SymbolKind>,
    /// Filter by file path (substring match, None = all).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_filter: Option<String>,
}

fn default_limit() -> usize { 50 }

/// A search result match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    /// The matched symbol.
    pub symbol: crate::symbol::Symbol,
    /// Match score (higher = better).
    pub score: f32,
    /// Matched positions in the symbol name.
    pub matched_positions: Vec<usize>,
}

/// A search match detail (for highlighting).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchMatch {
    /// Start index in the name.
    pub start: usize,
    /// End index (exclusive).
    pub end: usize,
}

/// Perform fuzzy search over a list of symbols.
pub fn fuzzy_search(
    query: &str,
    symbols: &[crate::symbol::Symbol],
    limit: usize,
) -> Vec<SearchResult> {
    if query.is_empty() {
        return symbols
            .iter()
            .take(limit)
            .map(|s| SearchResult {
                symbol: s.clone(),
                score: 0.0,
                matched_positions: Vec::new(),
            })
            .collect();
    }

    let query_lower = query.to_lowercase();
    let mut results: Vec<SearchResult> = symbols
        .iter()
        .filter_map(|sym| {
            let name_lower = sym.name.to_lowercase();
            match fuzzy_match(&query_lower, &name_lower) {
                Some((score, positions)) => Some(SearchResult {
                    symbol: sym.clone(),
                    score,
                    matched_positions: positions,
                }),
                None => None,
            }
        })
        .collect();

    results.sort_by(|a, b| {
        b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);
    results
}

/// Check if `query` is a subsequence of `target` (both lowercased).
/// Returns a score and the matched positions.
fn fuzzy_match(query: &str, target: &str) -> Option<(f32, Vec<usize>)> {
    let query_chars: Vec<char> = query.chars().collect();
    let target_chars: Vec<char> = target.chars().collect();

    if query_chars.is_empty() {
        return Some((1.0, Vec::new()));
    }
    if query_chars.len() > target_chars.len() {
        return None;
    }

    let mut positions: Vec<usize> = Vec::new();
    let mut qi = 0usize;
    let mut score: f32 = 0.0;
    let mut prev_match: Option<usize> = None;

    for (ti, tc) in target_chars.iter().enumerate() {
        if qi >= query_chars.len() {
            break;
        }
        if tc == &query_chars[qi] {
            // Bonus for consecutive matches.
            if let Some(prev) = prev_match {
                if ti == prev + 1 {
                    score += 0.5;
                }
            }
            // Bonus for word boundary (after non-alphanumeric or at start).
            if ti == 0 || !target_chars[ti - 1].is_alphanumeric() {
                score += 0.3;
            }
            // Bonus for camelCase boundary.
            if ti > 0 && target_chars[ti - 1].is_lowercase() && tc.is_uppercase() {
                score += 0.3;
            }
            // Penalty for distance from previous match.
            if let Some(prev) = prev_match {
                score -= (ti - prev - 1) as f32 * 0.1;
            }
            positions.push(ti);
            prev_match = Some(ti);
            qi += 1;
        }
    }

    if qi == query_chars.len() {
        // Bonus for matching early in the string.
        let first_pos = positions[0] as usize;
        score += (target_chars.len() - first_pos) as f32 / target_chars.len() as f32 * 2.0;
        // Bonus for shorter target (more relevant).
        score += 1.0 / (target_chars.len() as f32 / 10.0 + 1.0);
        Some((score, positions))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::{Symbol, SymbolKind, SymbolLocation};
    use smol_str::SmolStr;

    fn make_symbol(name: &str) -> Symbol {
        Symbol::new(
            SmolStr::new(name),
            SymbolKind::Function,
            "rust",
            SymbolLocation { file: "test.rs".into(), line: 1, column: 1, end_line: 1 },
        )
    }

    #[test]
    fn fuzzy_match_finds_subsequence() {
        let symbols = vec![
            make_symbol("hello_world"),
            make_symbol("goodbye"),
            make_symbol("help"),
            make_symbol("world_hello"),
        ];
        let results = fuzzy_search("hel", &symbols, 10);
        assert!(results.iter().any(|r| r.symbol.name == "hello_world"));
        assert!(results.iter().any(|r| r.symbol.name == "help"));
        assert!(!results.iter().any(|r| r.symbol.name == "goodbye"));
    }

    #[test]
    fn fuzzy_match_scores_word_boundary_higher() {
        let symbols = vec![
            make_symbol("myFunction"),   // camelCase boundary
            make_symbol("xmyFunction"),  // not at boundary
        ];
        let results = fuzzy_search("myF", &symbols, 10);
        // The one with the boundary should score higher.
        assert_eq!(results[0].symbol.name, "myFunction");
    }

    #[test]
    fn fuzzy_match_returns_empty_for_no_match() {
        let symbols = vec![make_symbol("hello")];
        let results = fuzzy_search("xyz", &symbols, 10);
        assert!(results.is_empty());
    }

    #[test]
    fn fuzzy_match_case_insensitive() {
        let symbols = vec![make_symbol("HelloWorld")];
        let results = fuzzy_search("hw", &symbols, 10);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn fuzzy_match_limits_results() {
        let symbols: Vec<Symbol> = (0..100).map(|i| make_symbol(&format!("func_{}", i))).collect();
        let results = fuzzy_search("func", &symbols, 10);
        assert_eq!(results.len(), 10);
    }

    #[test]
    fn fuzzy_match_empty_query_returns_all() {
        let symbols = vec![make_symbol("a"), make_symbol("b")];
        let results = fuzzy_search("", &symbols, 10);
        assert_eq!(results.len(), 2);
    }
}
