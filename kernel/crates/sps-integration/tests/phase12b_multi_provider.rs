//! Phase 12B: Multi-provider LLM system tests.
//!
//! Verifies:
//! 1. 12+ built-in provider templates available
//! 2. Custom provider can be registered via AddCustomProviderRequest
//! 3. ProviderLlmAdapter bridges to ProviderRegistry
//! 4. LLM failure handled gracefully (provider not registered)
//! 5. build_provider creates correct adapter type per ApiFormat
//! 6. Factory integration: ProviderLlmAdapter used as LlmFactoryAdapter

use std::sync::Arc;

use smol_str::SmolStr;
use sps_effects::providers::llm::{LlmProvider, ProviderConfig};
use sps_effects::providers::registry::ProviderRegistry;
use sps_providers_http::{
    AddCustomProviderRequest, ApiFormat, ProviderTemplate, build_provider, builtin_templates,
    get_builtin_template,
};
use sps_factory::llm::LlmFactoryAdapter;
use sps_factory::provider_llm::ProviderLlmAdapter;
use sps_factory::workflow::ProjectRequest;

#[test]
fn phase12b_test_1_12_builtin_templates_available() {
    println!("\n=== Phase 12B Test 1: 12+ built-in templates ===");
    let templates = builtin_templates();
    println!("  Built-in templates: {} providers", templates.len());
    for t in &templates {
        println!("    - {} ({:?}): {}", t.id, t.api_format, t.default_api_url);
    }

    assert!(templates.len() >= 12, "FAIL: expected >=12 templates, got {}", templates.len());

    // Verify key providers are present.
    let ids: Vec<_> = templates.iter().map(|t| t.id.as_str()).collect();
    for expected in &["openai", "anthropic", "openrouter", "groq", "deepseek",
                     "mistral", "cohere", "together", "fireworks",
                     "ollama", "lmstudio", "vllm", "azure-openai"] {
        assert!(ids.contains(expected), "FAIL: missing provider template '{}'", expected);
    }
    println!("  PASS — All 13 expected providers present");
}

#[test]
fn phase12b_test_2_get_builtin_template_by_id() {
    println!("\n=== Phase 12B Test 2: get_builtin_template by id ===");
    let openai = get_builtin_template("openai").unwrap();
    assert_eq!(openai.api_format, ApiFormat::OpenAi);
    assert!(openai.requires_api_key);
    assert_eq!(openai.auth_header, "Authorization");
    assert_eq!(openai.auth_prefix, "Bearer ");

    let anthropic = get_builtin_template("anthropic").unwrap();
    assert_eq!(anthropic.api_format, ApiFormat::Anthropic);
    assert_eq!(anthropic.auth_header, "x-api-key");

    let ollama = get_builtin_template("ollama").unwrap();
    assert_eq!(ollama.api_format, ApiFormat::Ollama);
    assert!(!ollama.requires_api_key);

    assert!(get_builtin_template("nonexistent").is_none());
    println!("  PASS — Template lookup works for all formats");
}

#[test]
fn phase12b_test_3_custom_provider_request() {
    println!("\n=== Phase 12B Test 3: Custom provider via AddCustomProviderRequest ===");
    let req = AddCustomProviderRequest {
        id: "my-custom-llm".into(),
        name: "My Custom LLM Service".into(),
        api_url: "https://my-llm.example.com/v1".into(),
        api_key: Some("sk-custom-key".into()),
        model_name: "custom-model-v2".into(),
        api_format: ApiFormat::OpenAi,
        endpoint_path: "/chat/completions".into(),
        auth_header: "Authorization".into(),
        auth_prefix: "Bearer ".into(),
        extra_headers: std::collections::BTreeMap::new(),
    };

    let template = req.to_template();
    assert_eq!(template.id, "my-custom-llm");
    assert!(!template.builtin, "FAIL: custom template should have builtin=false");
    assert_eq!(template.api_format, ApiFormat::OpenAi);

    let config = req.to_config();
    assert_eq!(config.id, "my-custom-llm");
    assert_eq!(config.api_url, "https://my-llm.example.com/v1");
    assert!(config.api_key.is_some());

    println!("  PASS — Custom provider template + config generated correctly");
}

#[test]
fn phase12b_test_4_build_provider_creates_correct_adapter() {
    println!("\n=== Phase 12B Test 4: build_provider creates correct adapter ===");

    // OpenAI format → HttpProviderAdapter.
    let template = get_builtin_template("openai").unwrap();
    let config = ProviderConfig {
        id: "test-openai".into(),
        name: "Test OpenAI".into(),
        api_url: "https://api.openai.com/v1".into(),
        api_key: Some("sk-test".into()),
        model_name: "gpt-4o".into(),
        metadata: Default::default(),
    };
    let provider = build_provider(&template, config).unwrap();
    assert_eq!(provider.id(), "test-openai");
    println!("  PASS — OpenAi format → HttpProviderAdapter");

    // Anthropic format → AnthropicAdapter.
    let template = get_builtin_template("anthropic").unwrap();
    let config = ProviderConfig {
        id: "test-anthropic".into(),
        name: "Test Anthropic".into(),
        api_url: "https://api.anthropic.com".into(),
        api_key: Some("sk-test".into()),
        model_name: "claude-sonnet-4-20250514".into(),
        metadata: Default::default(),
    };
    let provider = build_provider(&template, config).unwrap();
    // AnthropicAdapter has a hardcoded id "anthropic".
    assert_eq!(provider.id(), "anthropic");
    println!("  PASS — Anthropic format → AnthropicAdapter");

    // Ollama format → OllamaAdapter.
    let template = get_builtin_template("ollama").unwrap();
    let config = ProviderConfig {
        id: "test-ollama".into(),
        name: "Test Ollama".into(),
        api_url: "http://localhost:11434".into(),
        api_key: None,
        model_name: "llama3.2".into(),
        metadata: Default::default(),
    };
    let provider = build_provider(&template, config).unwrap();
    // OllamaAdapter has a hardcoded id "ollama".
    assert_eq!(provider.id(), "ollama");
    println!("  PASS — Ollama format → OllamaAdapter");
}

#[test]
fn phase12b_test_5_provider_llm_adapter_handles_missing_provider() {
    println!("\n=== Phase 12B Test 5: ProviderLlmAdapter handles missing provider ===");
    let registry = Arc::new(ProviderRegistry::new());
    // Note: no providers registered — should fail gracefully.
    let adapter = ProviderLlmAdapter::new(registry, "nonexistent".into());

    let request = ProjectRequest {
        description: "test".into(),
        preferred_name: None,
        output_dir: None,
    };
    let result = adapter.analyze_requirement(&request);
    assert!(result.is_err(), "FAIL: expected error when provider not registered");
    let err = result.unwrap_err();
    assert!(
        err.contains("not registered"),
        "FAIL: error should mention 'not registered', got: {}",
        err
    );
    println!("  PASS — Missing provider returns error: {}", err);
}

#[test]
fn phase12b_test_6_provider_llm_adapter_name() {
    println!("\n=== Phase 12B Test 6: ProviderLlmAdapter name ===");
    let registry = Arc::new(ProviderRegistry::new());
    let adapter = ProviderLlmAdapter::new(registry, "openai".into());
    assert_eq!(adapter.name(), "provider-llm");
    println!("  PASS — Adapter name = 'provider-llm'");
}

#[test]
fn phase12b_test_7_registry_supports_multiple_providers() {
    println!("\n=== Phase 12B Test 7: Registry supports multiple providers ===");
    let registry = Arc::new(ProviderRegistry::new());

    // Register 3 providers with different templates.
    for (kind, url) in &[
        ("openai", "https://api.openai.com/v1"),
        ("anthropic", "https://api.anthropic.com"),
        ("ollama", "http://localhost:11434"),
    ] {
        let template = get_builtin_template(kind).unwrap();
        let config = ProviderConfig {
            id: (*kind).into(),
            name: (*kind).into(),
            api_url: (*url).into(),
            api_key: if template.requires_api_key { Some("key".into()) } else { None },
            model_name: template.default_model.clone(),
            metadata: Default::default(),
        };
        let provider = build_provider(&template, config.clone()).unwrap();
        registry.register(config, provider);
    }

    assert_eq!(registry.len(), 3, "FAIL: expected 3 providers, got {}", registry.len());
    let ids = registry.list();
    assert!(ids.contains(&SmolStr::new("openai")));
    assert!(ids.contains(&SmolStr::new("anthropic")));
    assert!(ids.contains(&SmolStr::new("ollama")));
    println!("  PASS — 3 providers registered: {:?}", ids);

    // Verify lookup works.
    assert!(registry.get("openai").is_some());
    assert!(registry.get("anthropic").is_some());
    assert!(registry.get("ollama").is_some());
    assert!(registry.get("nonexistent").is_none());
    println!("  PASS — Provider lookup works for all registered providers");
}

#[test]
fn phase12b_test_8_custom_provider_with_anthropic_format() {
    println!("\n=== Phase 12B Test 8: Custom provider with Anthropic format ===");
    let req = AddCustomProviderRequest {
        id: "custom-claude-proxy".into(),
        name: "Custom Claude Proxy".into(),
        api_url: "https://my-proxy.example.com".into(),
        api_key: Some("sk-proxy".into()),
        model_name: "claude-sonnet-4-20250514".into(),
        api_format: ApiFormat::Anthropic,
        endpoint_path: "/v1/messages".into(),
        auth_header: "x-api-key".into(),
        auth_prefix: "".into(),
        extra_headers: [("anthropic-version".to_string(), "2023-06-01".to_string())]
            .into_iter()
            .collect(),
    };

    let template = req.to_template();
    assert_eq!(template.api_format, ApiFormat::Anthropic);
    assert_eq!(template.auth_header, "x-api-key");
    assert!(template.extra_headers.contains_key("anthropic-version"));

    let config = req.to_config();
    let provider = build_provider(&template, config).unwrap();
    // AnthropicAdapter has a hardcoded id "anthropic".
    assert_eq!(provider.id(), "anthropic");
    println!("  PASS — Custom Anthropic-format provider created successfully");
}
