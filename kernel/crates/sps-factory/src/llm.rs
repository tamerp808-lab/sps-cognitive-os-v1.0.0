//! Phase 11D: LLM-powered Factory stages.
//!
//! Makes 3 of the 8 factory stages LLM-driven:
//! - RequirementAnalysis: LLM parses natural language → RequirementSpec
//! - ArchitectureDesign: LLM generates ArchitecturePlan from spec
//! - CodeGeneration: LLM generates source files from architecture
//!
//! The remaining 5 stages stay deterministic:
//! - Planning (no-op — Phase 7)
//! - Testing (effect-based, deterministic in dry_run)
//! - Validation (file non-empty check + build effect)
//! - Packaging (effect-based, deterministic in dry_run)
//! - DeploymentPrep (template-based Dockerfile + CI config)
//!
//! The adapter is pluggable: production uses sps-providers-http LLM
//! providers, tests use MockLlmAdapter (deterministic).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use sps_execution::generation::GeneratedFile;
use crate::workflow::{ArchitecturePlan, ProjectRequest, RequirementSpec};

/// Trait for LLM-powered factory stages.
///
/// Implementations:
/// - `MockLlmAdapter`: deterministic, for tests
/// - (Future) `ProviderLlmAdapter`: uses sps-providers-http for real LLM calls
pub trait LlmFactoryAdapter: Send + Sync + 'static {
    /// Stage 1: Analyze a natural-language request into a structured spec.
    fn analyze_requirement(&self, request: &ProjectRequest) -> Result<RequirementSpec, String>;

    /// Stage 2: Design an architecture from a requirement spec.
    fn design_architecture(&self, spec: &RequirementSpec) -> Result<ArchitecturePlan, String>;

    /// Stage 4: Generate source code from a spec + architecture.
    fn generate_code(&self, spec: &RequirementSpec, arch: &ArchitecturePlan, output_dir: &str) -> Result<Vec<GeneratedFile>, String>;

    /// Human-readable name (for logging).
    fn name(&self) -> &str;
}

/// Mock LLM adapter — deterministic, no network calls.
/// Used in tests and as a fallback when no LLM provider is configured.
#[derive(Debug, Default)]
pub struct MockLlmAdapter {
    /// If true, the adapter simulates an LLM failure on the next call.
    /// Used to test error handling.
    pub simulate_failure: bool,
}

impl MockLlmAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_failure() -> Self {
        Self { simulate_failure: true }
    }
}

impl LlmFactoryAdapter for MockLlmAdapter {
    fn analyze_requirement(&self, request: &ProjectRequest) -> Result<RequirementSpec, String> {
        if self.simulate_failure {
            return Err("mock LLM: simulated failure".into());
        }

        // Deterministic mock: parse keywords from the description.
        let name = request.preferred_name.clone().unwrap_or_else(|| SmolStr::new("llm-project"));
        let kind = if request.description.contains("rust") {
            "rust_cli"
        } else if request.description.contains("next") || request.description.contains("react") {
            "nextjs"
        } else if request.description.contains("tauri") {
            "tauri"
        } else {
            "rust_cli"
        };

        let mut requirements = Vec::new();
        if request.description.contains("cli") {
            requirements.push("Command-line interface".into());
        }
        if request.description.contains("api") {
            requirements.push("REST API".into());
        }
        if request.description.contains("auth") {
            requirements.push("Authentication".into());
        }
        if requirements.is_empty() {
            requirements.push("Core functionality".into());
        }

        Ok(RequirementSpec {
            name,
            kind: kind.into(),
            requirements,
            non_functional: vec!["Cross-platform".into(), "Testable".into()],
        })
    }

    fn design_architecture(&self, spec: &RequirementSpec) -> Result<ArchitecturePlan, String> {
        if self.simulate_failure {
            return Err("mock LLM: simulated failure".into());
        }

        let stack = match spec.kind.as_str() {
            "rust_cli" | "rust_lib" => vec!["rust".into(), "cargo".into(), "clap".into()],
            "nextjs" => vec!["typescript".into(), "next.js".into(), "react".into(), "tailwind".into()],
            "tauri" => vec!["rust".into(), "typescript".into(), "tauri".into()],
            _ => vec!["unknown".into()],
        };

        let file_layout = match spec.kind.as_str() {
            "rust_cli" => vec![
                "Cargo.toml".into(),
                "src/main.rs".into(),
                "src/cli.rs".into(),
                "src/lib.rs".into(),
                "tests/integration.rs".into(),
                "README.md".into(),
            ],
            "rust_lib" => vec!["Cargo.toml".into(), "src/lib.rs".into()],
            "nextjs" => vec![
                "package.json".into(),
                "src/app/page.tsx".into(),
                "src/app/layout.tsx".into(),
                "src/lib/api.ts".into(),
            ],
            "tauri" => vec![
                "package.json".into(),
                "src/app/page.tsx".into(),
                "src-tauri/Cargo.toml".into(),
                "src-tauri/src/main.rs".into(),
            ],
            _ => vec!["README.md".into()],
        };

        Ok(ArchitecturePlan {
            stack,
            file_layout,
            dependencies: vec![],
        })
    }

    fn generate_code(&self, spec: &RequirementSpec, _arch: &ArchitecturePlan, _output_dir: &str) -> Result<Vec<GeneratedFile>, String> {
        if self.simulate_failure {
            return Err("mock LLM: simulated failure".into());
        }

        // Deterministic mock: generate minimal files based on kind.
        let mut files = Vec::new();

        match spec.kind.as_str() {
            "rust_cli" => {
                files.push(GeneratedFile {
                    path: "Cargo.toml".into(),
                    content: format!(
                        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nclap = {{ version = \"4\", features = [\"derive\"] }}\n",
                        spec.name
                    ),
                });
                files.push(GeneratedFile {
                    path: "src/main.rs".into(),
                    content: format!(
                        "use clap::Parser;\n\n#[derive(Parser)]\n#[command(name = \"{}\")]\nstruct Cli {{\n    #[arg(short, long)]\n    name: Option<String>,\n}}\n\nfn main() {{\n    let cli = Cli::parse();\n    println!(\"Hello, {{}}!\", cli.name.unwrap_or(\"world\".into()));\n}}\n",
                        spec.name
                    ),
                });
                files.push(GeneratedFile {
                    path: "README.md".into(),
                    content: format!("# {}\n\nGenerated by SPS Factory (Phase 11D LLM mock).\n", spec.name),
                });
            }
            "nextjs" => {
                files.push(GeneratedFile {
                    path: "package.json".into(),
                    content: format!(
                        "{{\"name\":\"{}\",\"version\":\"0.1.0\",\"scripts\":{{\"dev\":\"next dev\",\"build\":\"next build\",\"start\":\"next start\"}}}}\n",
                        spec.name
                    ),
                });
                files.push(GeneratedFile {
                    path: "src/app/page.tsx".into(),
                    content: "export default function Home() {\n  return <h1>Hello from SPS Factory</h1>;\n}\n".into(),
                });
            }
            _ => {
                files.push(GeneratedFile {
                    path: "README.md".into(),
                    content: format!("# {}\n\nGenerated by SPS Factory.\n", spec.name),
                });
            }
        }

        Ok(files)
    }

    fn name(&self) -> &str {
        "mock-llm"
    }
}

/// Configuration for LLM-powered factory stages.
#[derive(Clone)]
pub struct LlmFactoryConfig {
    /// The LLM adapter. If None, falls back to deterministic string-matching.
    pub adapter: Option<Arc<dyn LlmFactoryAdapter>>,
}

impl Default for LlmFactoryConfig {
    fn default() -> Self {
        Self { adapter: None }
    }
}

impl LlmFactoryConfig {
    /// Create a config with no LLM (deterministic fallback).
    pub fn deterministic() -> Self {
        Self::default()
    }

    /// Create a config with the given LLM adapter.
    pub fn with_adapter(adapter: Arc<dyn LlmFactoryAdapter>) -> Self {
        Self { adapter: Some(adapter) }
    }

    /// Create a config with the MockLlmAdapter.
    pub fn with_mock() -> Self {
        Self::with_adapter(Arc::new(MockLlmAdapter::new()))
    }

    /// Returns true if an LLM adapter is configured.
    pub fn has_llm(&self) -> bool {
        self.adapter.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_adapter_analyzes_requirement() {
        let adapter = MockLlmAdapter::new();
        let request = ProjectRequest {
            description: "build a rust cli tool with auth".into(),
            preferred_name: Some(SmolStr::new("my-cli")),
            output_dir: None,
        };
        let spec = adapter.analyze_requirement(&request).unwrap();
        assert_eq!(spec.name, "my-cli");
        assert_eq!(spec.kind, "rust_cli");
        assert!(spec.requirements.iter().any(|r| r.contains("Authentication")));
    }

    #[test]
    fn mock_adapter_designs_architecture() {
        let adapter = MockLlmAdapter::new();
        let spec = RequirementSpec {
            name: "test".into(),
            kind: "rust_cli".into(),
            requirements: vec![],
            non_functional: vec![],
        };
        let arch = adapter.design_architecture(&spec).unwrap();
        assert!(arch.stack.iter().any(|s| s == "rust"));
        assert!(arch.file_layout.contains(&"Cargo.toml".to_string()));
    }

    #[test]
    fn mock_adapter_generates_code() {
        let adapter = MockLlmAdapter::new();
        let spec = RequirementSpec {
            name: "test".into(),
            kind: "rust_cli".into(),
            requirements: vec![],
            non_functional: vec![],
        };
        let arch = ArchitecturePlan {
            stack: vec![],
            file_layout: vec![],
            dependencies: vec![],
        };
        let files = adapter.generate_code(&spec, &arch, "/tmp").unwrap();
        assert!(!files.is_empty());
        assert!(files.iter().any(|f| f.path == "Cargo.toml"));
    }

    #[test]
    fn mock_adapter_simulates_failure() {
        let adapter = MockLlmAdapter::with_failure();
        let request = ProjectRequest {
            description: "test".into(),
            preferred_name: None,
            output_dir: None,
        };
        let result = adapter.analyze_requirement(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("simulated failure"));
    }
}
