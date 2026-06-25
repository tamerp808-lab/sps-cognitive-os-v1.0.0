//! Phase 9 — Reflection tests.

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_reflection::analyzers::{
    FailureAnalyzer, KnowledgeConsolidator, PatternExtractor, RootCause, SuccessAnalyzer,
};
use sps_reflection::reducer::{ReflectionReducer, ReflectionState, Reflection};

fn fresh_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    ReflectionReducer::register(&mut reg);
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
}

#[test]
fn success_analyzer_marks_generalizable() {
    let id = uuid::Uuid::now_v7();
    let a = SuccessAnalyzer::analyze(
        id,
        vec!["fast iteration".into()],
        "approach worked well".into(),
        true,
    );
    assert!(a.generalizable);
    assert!(a.pattern_name.is_some());
}

#[test]
fn failure_analyzer_classifies_provider_issue() {
    let id = uuid::Uuid::now_v7();
    let a = FailureAnalyzer::analyze(id, "no provider available for LLM effect");
    assert_eq!(a.root_cause, RootCause::ProviderIssue);
    assert!(a.suggested_fix.contains("provider"));
}

#[test]
fn failure_analyzer_classifies_timeout() {
    let id = uuid::Uuid::now_v7();
    let a = FailureAnalyzer::analyze(id, "operation timeout after 30s");
    assert_eq!(a.root_cause, RootCause::Timeout);
}

#[test]
fn failure_analyzer_classifies_ambiguity() {
    let id = uuid::Uuid::now_v7();
    let a = FailureAnalyzer::analyze(id, "goal is ambiguous");
    assert_eq!(a.root_cause, RootCause::Ambiguity);
}

#[test]
fn pattern_extractor_groups_by_root_cause() {
    let items = vec![
        (RootCause::Timeout, 5),
        (RootCause::ProviderIssue, 3),
    ];
    let patterns = PatternExtractor::extract(&items);
    assert_eq!(patterns.len(), 2);
    assert_eq!(patterns[0].count, 5);
    assert!(patterns[0].confidence > patterns[1].confidence);
}

#[test]
fn knowledge_consolidator_filters_by_confidence() {
    let patterns = vec![
        sps_reflection::analyzers::Pattern {
            name: "strong".into(),
            description: "x".into(),
            count: 10,
            confidence: 0.9,
        },
        sps_reflection::analyzers::Pattern {
            name: "weak".into(),
            description: "y".into(),
            count: 1,
            confidence: 0.1,
        },
    ];
    let consolidated = KnowledgeConsolidator::consolidate(&patterns, 0.5);
    assert_eq!(consolidated.len(), 1);
    assert_eq!(consolidated[0].name, "strong");
}

#[test]
fn reflection_success_event_persists() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let id = uuid::Uuid::now_v7();
    let analysis = SuccessAnalyzer::analyze(id, vec!["x".into()], "y".into(), true);
    let event = RawEvent::new(
        "reflection.success_analyzed",
        serde_json::to_value(&analysis).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let rs = ReflectionState::from_state(&state).unwrap();
    assert_eq!(rs.reflections.len(), 1);
    assert!(matches!(
        rs.reflections.values().next().unwrap(),
        Reflection::Success(_)
    ));
}

#[test]
fn reflection_failure_event_persists() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let id = uuid::Uuid::now_v7();
    let analysis = FailureAnalyzer::analyze(id, "no provider available");
    let event = RawEvent::new(
        "reflection.failure_analyzed",
        serde_json::to_value(&analysis).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let rs = ReflectionState::from_state(&state).unwrap();
    assert_eq!(rs.reflections.len(), 1);
    assert!(matches!(
        rs.reflections.values().next().unwrap(),
        Reflection::Failure(_)
    ));
}
