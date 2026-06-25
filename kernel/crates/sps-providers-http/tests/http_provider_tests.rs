//! Phase 1.5 — HTTP providers tests.
//!
//! These tests use `wiremock` to spin up a mock HTTP server so we can
//! verify the providers work end-to-end without hitting real APIs.
//!
//! Tests are sync (`#[test]`) because the providers themselves are
//! sync — they spawn their own tokio runtime internally. We use a
//! helper `block_on` to drive the async mock-server setup.

use sps_effects::providers::llm::{LlmProvider, LlmRequest, ProviderConfig};
use sps_providers_http::{AnthropicAdapter, HttpProviderAdapter, OllamaAdapter};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build runtime");
    rt.block_on(f)
}

fn make_config(api_url: String, api_key: Option<String>, model: &str) -> ProviderConfig {
    ProviderConfig {
        id: "test".into(),
        name: "Test".into(),
        api_url,
        api_key,
        model_name: model.into(),
        metadata: Default::default(),
    }
}

#[test]
fn http_adapter_completes_against_mock_server() {
    let server = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {"role": "assistant", "content": "Hello from mock!"}
                }],
                "usage": {"prompt_tokens": 5, "completion_tokens": 4, "total_tokens": 9}
            })))
            .mount(&server)
            .await;
        server
    });

    let adapter = HttpProviderAdapter::new("test", "/v1/chat/completions");
    adapter.configure(make_config(server.uri(), Some("sk-test".into()), "gpt-4"));

    let req = LlmRequest {
        provider_id: "test".into(),
        model: None,
        system: Some("You are helpful.".into()),
        user: "Say hello.".into(),
        max_tokens: Some(50),
        temperature: Some(0.7),
    };
    let completion = adapter.complete(&req).expect("completion should succeed");
    assert_eq!(completion.text, "Hello from mock!");
    assert_eq!(completion.usage.total_tokens, 9);
}

#[test]
fn http_adapter_healthcheck_succeeds_on_200() {
    let server = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
            .mount(&server)
            .await;
        server
    });

    let adapter = HttpProviderAdapter::new("test", "/v1/chat/completions");
    adapter.configure(make_config(server.uri(), Some("sk-test".into()), "gpt-4"));
    let health = adapter.healthcheck().expect("healthcheck should succeed");
    assert!(health.healthy);
}

#[test]
fn http_adapter_healthcheck_fails_on_500() {
    let server = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        server
    });

    let adapter = HttpProviderAdapter::new("test", "/v1/chat/completions");
    adapter.configure(make_config(server.uri(), Some("sk-test".into()), "gpt-4"));
    let health = adapter.healthcheck().expect("healthcheck should not panic");
    assert!(!health.healthy);
    assert!(health.error.is_some());
}

#[test]
fn http_adapter_retries_on_5xx() {
    let server = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"role": "assistant", "content": "recovered"}}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
            })))
            .mount(&server)
            .await;
        server
    });

    let adapter = HttpProviderAdapter::new("test", "/v1/chat/completions")
        .with_retry(sps_providers_http::RetryConfig {
            max_retries: 3,
            initial_backoff_ms: 1,
            max_backoff_ms: 10,
            multiplier: 2.0,
        });
    adapter.configure(make_config(server.uri(), Some("sk-test".into()), "gpt-4"));

    let req = LlmRequest {
        provider_id: "test".into(),
        model: None,
        system: None,
        user: "hi".into(),
        max_tokens: None,
        temperature: None,
    };
    let completion = adapter.complete(&req).expect("should succeed after retry");
    assert_eq!(completion.text, "recovered");
}

#[test]
fn http_adapter_fails_on_4xx() {
    let server = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server)
            .await;
        server
    });

    let adapter = HttpProviderAdapter::new("test", "/v1/chat/completions");
    adapter.configure(make_config(server.uri(), Some("bad-key".into()), "gpt-4"));

    let req = LlmRequest {
        provider_id: "test".into(),
        model: None,
        system: None,
        user: "hi".into(),
        max_tokens: None,
        temperature: None,
    };
    let result = adapter.complete(&req);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("401") || err.contains("HTTP"));
}

#[test]
fn anthropic_adapter_completes_against_mock() {
    let server = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type": "text", "text": "Claude says hi"}],
                "usage": {"input_tokens": 3, "output_tokens": 4}
            })))
            .mount(&server)
            .await;
        server
    });

    let adapter = AnthropicAdapter::new();
    adapter.configure(make_config(server.uri(), Some("sk-ant".into()), "claude-3-5-sonnet"));

    let req = LlmRequest {
        provider_id: "anthropic".into(),
        model: None,
        system: Some("Be brief.".into()),
        user: "Hi".into(),
        max_tokens: Some(100),
        temperature: None,
    };
    let completion = adapter.complete(&req).expect("anthropic should complete");
    assert_eq!(completion.text, "Claude says hi");
    assert_eq!(completion.usage.prompt_tokens, 3);
    assert_eq!(completion.usage.completion_tokens, 4);
}

#[test]
fn ollama_adapter_completes_against_mock() {
    let server = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"role": "assistant", "content": "Ollama local response"},
                "done": true,
                "prompt_eval_count": 8,
                "eval_count": 5
            })))
            .mount(&server)
            .await;
        server
    });

    let adapter = OllamaAdapter::new();
    adapter.configure(make_config(server.uri(), None, "llama3.2"));

    let req = LlmRequest {
        provider_id: "ollama".into(),
        model: None,
        system: None,
        user: "Hello".into(),
        max_tokens: None,
        temperature: None,
    };
    let completion = adapter.complete(&req).expect("ollama should complete");
    assert_eq!(completion.text, "Ollama local response");
    assert_eq!(completion.usage.prompt_tokens, 8);
    assert_eq!(completion.usage.completion_tokens, 5);
}

#[test]
fn ollama_adapter_healthcheck_succeeds() {
    let server = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"models": []})))
            .mount(&server)
            .await;
        server
    });

    let adapter = OllamaAdapter::new();
    adapter.configure(make_config(server.uri(), None, "llama3.2"));
    let health = adapter.healthcheck().expect("healthcheck should not panic");
    assert!(health.healthy);
}

#[test]
fn retry_policy_delay_for_exponential() {
    use std::time::Duration;
    use sps_providers_http::RetryPolicy;
    let policy = RetryPolicy::new(sps_providers_http::RetryConfig {
        max_retries: 5,
        initial_backoff_ms: 100,
        max_backoff_ms: 10_000,
        multiplier: 2.0,
    });
    assert_eq!(policy.delay_for(0), Some(Duration::from_millis(100)));
    assert_eq!(policy.delay_for(1), Some(Duration::from_millis(200)));
    assert_eq!(policy.delay_for(2), Some(Duration::from_millis(400)));
    assert_eq!(policy.delay_for(5), None);
}

#[test]
fn retry_policy_caps_at_max() {
    use std::time::Duration;
    use sps_providers_http::RetryPolicy;
    let policy = RetryPolicy::new(sps_providers_http::RetryConfig {
        max_retries: 10,
        initial_backoff_ms: 100,
        max_backoff_ms: 1000,
        multiplier: 2.0,
    });
    assert_eq!(policy.delay_for(4), Some(Duration::from_millis(1000)));
}

