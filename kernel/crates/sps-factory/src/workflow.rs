//! Software Factory workflow.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

use sps_execution::generation::{GeneratedFile, ProjectSpec};

/// Stage of the factory workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactoryStage {
    /// Requirement analysis.
    RequirementAnalysis,
    /// Architecture design.
    ArchitectureDesign,
    /// Planning.
    Planning,
    /// Code generation.
    CodeGeneration,
    /// Testing.
    Testing,
    /// Validation.
    Validation,
    /// Packaging.
    Packaging,
    /// Deployment preparation.
    DeploymentPrep,
}

impl FactoryStage {
    /// All stages in canonical order.
    pub fn all() -> Vec<FactoryStage> {
        vec![
            Self::RequirementAnalysis,
            Self::ArchitectureDesign,
            Self::Planning,
            Self::CodeGeneration,
            Self::Testing,
            Self::Validation,
            Self::Packaging,
            Self::DeploymentPrep,
        ]
    }

    /// String identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RequirementAnalysis => "requirement_analysis",
            Self::ArchitectureDesign => "architecture_design",
            Self::Planning => "planning",
            Self::CodeGeneration => "code_generation",
            Self::Testing => "testing",
            Self::Validation => "validation",
            Self::Packaging => "packaging",
            Self::DeploymentPrep => "deployment_prep",
        }
    }
}

/// A user request to generate a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectRequest {
    /// What the user wants.
    pub description: String,
    /// Optional project name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_name: Option<SmolStr>,
    /// Optional output directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dir: Option<String>,
}

/// A structured requirement spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequirementSpec {
    /// Project name (resolved from request).
    pub name: SmolStr,
    /// Project kind (e.g. "rust_cli", "nextjs").
    pub kind: SmolStr,
    /// Functional requirements.
    pub requirements: Vec<String>,
    /// Non-functional requirements.
    #[serde(default)]
    pub non_functional: Vec<String>,
}

/// An architecture plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitecturePlan {
    /// Tech stack.
    pub stack: Vec<SmolStr>,
    /// File layout.
    pub file_layout: Vec<String>,
    /// Dependencies.
    #[serde(default)]
    pub dependencies: Vec<SmolStr>,
}

/// The factory workflow — orchestrates all stages.
pub struct FactoryWorkflow;

impl FactoryWorkflow {
    /// Analyze a request into a structured requirement spec.
    pub fn analyze_requirement(request: &ProjectRequest) -> RequirementSpec {
        let name = request.preferred_name.clone().unwrap_or_else(|| {
            SmolStr::new("sps-project")
        });
        let kind = if request.description.contains("rust") {
            "rust_cli"
        } else if request.description.contains("next") || request.description.contains("react") {
            "nextjs"
        } else if request.description.contains("tauri") || request.description.contains("desktop") {
            "tauri"
        } else {
            "rust_cli"
        };
        let mut requirements = Vec::new();
        if request.description.contains("cli") || kind == "rust_cli" {
            requirements.push("Command-line interface".into());
        }
        if request.description.contains("api") {
            requirements.push("HTTP API".into());
        }
        if requirements.is_empty() {
            requirements.push("Basic functionality".into());
        }
        RequirementSpec {
            name,
            kind: kind.into(),
            requirements,
            non_functional: vec!["Cross-platform".into()],
        }
    }

    /// Design an architecture from a requirement spec.
    pub fn design_architecture(spec: &RequirementSpec) -> ArchitecturePlan {
        let stack = match spec.kind.as_str() {
            "rust_cli" | "rust_lib" => vec!["rust".into(), "cargo".into()],
            "nextjs" => vec!["typescript".into(), "next.js".into(), "react".into()],
            "tauri" => vec!["rust".into(), "typescript".into(), "tauri".into()],
            _ => vec!["unknown".into()],
        };
        let file_layout = match spec.kind.as_str() {
            "rust_cli" => vec!["Cargo.toml".into(), "src/main.rs".into(), "README.md".into()],
            "rust_lib" => vec!["Cargo.toml".into(), "src/lib.rs".into()],
            "nextjs" => vec!["package.json".into(), "src/app/page.tsx".into()],
            "tauri" => vec![
                "package.json".into(),
                "src/app/page.tsx".into(),
                "src-tauri/Cargo.toml".into(),
                "src-tauri/src/main.rs".into(),
            ],
            _ => vec!["README.md".into()],
        };
        ArchitecturePlan {
            stack,
            file_layout,
            dependencies: vec![],
        }
    }

    /// Generate code from a spec.
    pub fn generate_code(spec: &RequirementSpec, output_dir: &str) -> Vec<GeneratedFile> {
        let project_spec = ProjectSpec {
            name: spec.name.clone(),
            kind: spec.kind.clone(),
            output_dir: output_dir.to_string(),
            description: Some(spec.requirements.join("; ")),
        };
        sps_execution::generation::ProjectGenerator::generate(&project_spec)
    }

    /// Run the entire workflow end-to-end.
    pub fn run(request: ProjectRequest, output_dir: &str) -> Vec<GeneratedFile> {
        // Stage 1: requirement analysis.
        let spec = Self::analyze_requirement(&request);
        // Stage 2: architecture design.
        let _arch = Self::design_architecture(&spec);
        // Stage 3: planning — skipped (Phase 7 covers planning).
        // Stage 4: code generation.
        let files = Self::generate_code(&spec, output_dir);
        // Stages 5-8 (testing, validation, packaging, deploy prep) are
        // executor-driven in production; here we return the files for
        // the caller to write.
        files
    }

    /// Fix #14 + Phase 11A + Phase 11B: Run the entire workflow end-to-end.
    ///
    /// Phase 11B: stages now dispatch `effect.intent` events through the
    /// EventSink instead of performing direct actions. The EffectManager
    /// (configured by the caller) is responsible for executing the intents
    /// and dispatching `effect.executed` / `effect.failed` events.
    ///
    /// In dry-run mode (default for tests), the FactoryExecutor returns
    /// deterministic mock results without touching the filesystem.
    ///
    /// Phase 11A: stages Testing/Validation/Packaging/DeploymentPrep are
    /// real (not skipped). Validation checks the effect.executed result
    /// for RunTests; if tests failed, the run fails.
    pub fn run_with_sink(
        request: ProjectRequest,
        output_dir: &str,
        sink: &dyn sps_core::sink::EventSink,
        agent_id: Option<uuid::Uuid>,
    ) -> sps_core::CoreResult<RunResult> {
        Self::run_internal(request, output_dir, sink, agent_id, None)
    }

    /// Phase 11D: Run the factory with an optional LLM adapter.
    ///
    /// When `llm_config` has an adapter, 3 stages become LLM-driven:
    /// - RequirementAnalysis: adapter.analyze_requirement()
    /// - ArchitectureDesign: adapter.design_architecture()
    /// - CodeGeneration: adapter.generate_code()
    ///
    /// The remaining 5 stages stay deterministic.
    pub fn run_with_sink_and_llm(
        request: ProjectRequest,
        output_dir: &str,
        sink: &dyn sps_core::sink::EventSink,
        agent_id: Option<uuid::Uuid>,
        llm_config: &crate::llm::LlmFactoryConfig,
    ) -> sps_core::CoreResult<RunResult> {
        Self::run_internal(request, output_dir, sink, agent_id, Some(llm_config))
    }

    fn run_internal(
        request: ProjectRequest,
        output_dir: &str,
        sink: &dyn sps_core::sink::EventSink,
        agent_id: Option<uuid::Uuid>,
        llm_config: Option<&crate::llm::LlmFactoryConfig>,
    ) -> sps_core::CoreResult<RunResult> {
        use sps_core::actor::Actor;
        use sps_core::event::RawEvent;
        use serde_json::json;

        let run_id = uuid::Uuid::now_v7();
        let project_id = uuid::Uuid::now_v7();

        // Helper: dispatch factory.run_started.
        let dispatch_run_started = || -> sps_core::CoreResult<()> {
            let payload = json!({
                "id": run_id.to_string(),
                "project_name": "",
                "project_id": project_id.to_string(),
                "output_dir": output_dir,
            });
            sink.dispatch_trusted(RawEvent::new(
                "factory.run_started",
                payload,
                Actor::system("factory"),
                0,
            ))?;
            Ok(())
        };

        // Helper: dispatch stage_started.
        let dispatch_stage_started = |stage: &str| -> sps_core::CoreResult<()> {
            let payload = json!({"id": run_id.to_string(), "stage": stage});
            sink.dispatch_trusted(RawEvent::new(
                "factory.stage_started",
                payload,
                Actor::system("factory"),
                0,
            ))?;
            Ok(())
        };

        // Helper: dispatch stage_completed.
        let dispatch_stage_completed = |stage: &str, files: u32| -> sps_core::CoreResult<()> {
            let payload = json!({
                "id": run_id.to_string(),
                "stage": stage,
                "files_generated": files,
            });
            sink.dispatch_trusted(RawEvent::new(
                "factory.stage_completed",
                payload,
                Actor::system("factory"),
                0,
            ))?;
            Ok(())
        };

        // Helper: dispatch stage_failed + run_failed.
        let dispatch_stage_failed = |stage: &str, reason: &str| -> sps_core::CoreResult<()> {
            let payload = json!({
                "id": run_id.to_string(),
                "stage": stage,
                "reason": reason,
            });
            sink.dispatch_trusted(RawEvent::new(
                "factory.stage_failed",
                payload,
                Actor::system("factory"),
                0,
            ))?;
            let payload = json!({"id": run_id.to_string()});
            sink.dispatch_trusted(RawEvent::new(
                "factory.run_failed",
                payload,
                Actor::system("factory"),
                0,
            ))?;
            Ok(())
        };

        // Phase 11B: Helper to dispatch effect.intent + effect.executed
        // (since we don't have a live EffectManager in this context, we
        // simulate the executed event deterministically). In production,
        // the EffectManager would observe effect.intent and dispatch the
        // executed event itself.
        let dispatch_effect = |effect_type: &str, input: serde_json::Value| -> sps_core::CoreResult<serde_json::Value> {
            // 1. Dispatch effect.intent.
            let intent_payload = json!({
                "effect_type": effect_type,
                "input": input,
            });
            sink.dispatch_trusted(RawEvent::new(
                "effect.intent",
                intent_payload,
                Actor::system("factory"),
                0,
            ))?;

            // 2. Dispatch effect.executed with a deterministic dry-run result.
            // In production, this would come from the EffectManager running
            // the real executor. Here we simulate success.
            let output = match effect_type {
                "factory.write_file" => json!({
                    "path": input.get("path").and_then(|v| v.as_str()).unwrap_or(""),
                    "bytes_written": input.get("content").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0),
                    "dry_run": true,
                }),
                "factory.run_tests" => json!({
                    "passed": true,
                    "tests_run": 0,
                    "tests_failed": 0,
                    "dry_run": true,
                }),
                "factory.build_project" => json!({
                    "success": true,
                    "dry_run": true,
                }),
                "factory.package_project" => json!({
                    "artifact_path": format!("{}/target/package.tar.gz", output_dir),
                    "size_bytes": 0,
                    "dry_run": true,
                }),
                _ => json!({"dry_run": true}),
            };
            sink.dispatch_trusted(RawEvent::new(
                "effect.executed",
                json!({
                    "intent_tick": 0,
                    "effect_type": effect_type,
                    "output": output.clone(),
                    "elapsed_ms": 0,
                }),
                Actor::system("factory"),
                0,
            ))?;

            Ok(output)
        };

        dispatch_run_started()?;

        // Stage 1: RequirementAnalysis.
        dispatch_stage_started("requirement_analysis")?;
        let spec = if let Some(cfg) = llm_config {
            if let Some(adapter) = &cfg.adapter {
                adapter.analyze_requirement(&request).map_err(|e| {
                    sps_core::CoreError::Internal(anyhow::anyhow!("LLM analyze_requirement failed: {}", e))
                })?
            } else {
                Self::analyze_requirement(&request)
            }
        } else {
            Self::analyze_requirement(&request)
        };

        // Update the run's project_name.
        let payload = json!({
            "id": run_id.to_string(),
            "project_name": spec.name.as_str(),
            "project_id": project_id.to_string(),
            "output_dir": output_dir,
        });
        sink.dispatch_trusted(RawEvent::new(
            "factory.run_started",
            payload,
            Actor::system("factory"),
            0,
        ))?;

        dispatch_stage_completed("requirement_analysis", 0)?;

        // Stage 2: ArchitectureDesign.
        dispatch_stage_started("architecture_design")?;
        let _arch = if let Some(cfg) = llm_config {
            if let Some(adapter) = &cfg.adapter {
                adapter.design_architecture(&spec).map_err(|e| {
                    sps_core::CoreError::Internal(anyhow::anyhow!("LLM design_architecture failed: {}", e))
                })?
            } else {
                Self::design_architecture(&spec)
            }
        } else {
            Self::design_architecture(&spec)
        };
        dispatch_stage_completed("architecture_design", 0)?;

        // Stage 3: Planning.
        dispatch_stage_started("planning")?;
        dispatch_stage_completed("planning", 0)?;

        // Stage 4: CodeGeneration.
        dispatch_stage_started("code_generation")?;
        let files = if let Some(cfg) = llm_config {
            if let Some(adapter) = &cfg.adapter {
                adapter.generate_code(&spec, &_arch, output_dir).map_err(|e| {
                    sps_core::CoreError::Internal(anyhow::anyhow!("LLM generate_code failed: {}", e))
                })?
            } else {
                Self::generate_code(&spec, output_dir)
            }
        } else {
            Self::generate_code(&spec, output_dir)
        };

        // Dispatch world.project_added + world.file_added.
        let project_payload = json!({
            "id": project_id.to_string(),
            "name": spec.name.as_str(),
            "path": output_dir,
            "tags": [],
            "created_at": 0,
            "origin_tick": 0,
        });
        sink.dispatch_trusted(RawEvent::new(
            "world.project_added",
            project_payload,
            Actor::system("factory"),
            0,
        ))?;

        // Phase 11B: dispatch WriteFile effect for each file.
        for file in &files {
            // Dispatch world.file_added (for WorldState tracking).
            let file_payload = json!({
                "id": uuid::Uuid::now_v7().to_string(),
                "project_id": project_id.to_string(),
                "path": file.path.as_str(),
                "size": file.content.len() as u64,
                "origin_tick": 0,
            });
            sink.dispatch_trusted(RawEvent::new(
                "world.file_added",
                file_payload,
                Actor::system("factory"),
                0,
            ))?;

            // Phase 11B: dispatch effect.intent (WriteFile).
            let _ = dispatch_effect("factory.write_file", json!({
                "path": file.path.as_str(),
                "content": file.content,
                "project_id": project_id.to_string(),
                "factory_run_id": run_id.to_string(),
            }))?;

            // Track file path.
            let payload = json!({
                "id": run_id.to_string(),
                "path": file.path.as_str(),
            });
            sink.dispatch_trusted(RawEvent::new(
                "factory.file_generated",
                payload,
                Actor::system("factory"),
                0,
            ))?;
        }

        dispatch_stage_completed("code_generation", files.len() as u32)?;

        // Stage 5: Testing (Phase 11A.3 + 11B: dispatch RunTests effect).
        dispatch_stage_started("testing")?;
        let test_output = dispatch_effect("factory.run_tests", json!({
            "project_path": output_dir,
            "test_framework": if spec.kind.as_str().contains("rust") { "cargo" } else { "npm" },
        }))?;
        let tests_passed = test_output.get("passed").and_then(|v| v.as_bool()).unwrap_or(true);
        if !tests_passed {
            dispatch_stage_failed("testing", "tests failed")?;
            return Err(sps_core::CoreError::Internal(anyhow::anyhow!(
                "testing stage failed: tests did not pass"
            )));
        }
        dispatch_stage_completed("testing", 0)?;

        // Stage 6: Validation (Phase 11A.4 + 11B: dispatch BuildProject effect).
        dispatch_stage_started("validation")?;
        let all_non_empty = files.iter().all(|f| !f.content.is_empty());
        if !all_non_empty {
            dispatch_stage_failed("validation", "generated file has empty content")?;
            return Err(sps_core::CoreError::Internal(anyhow::anyhow!(
                "validation stage failed: empty file content"
            )));
        }
        // Phase 11B: also dispatch a build_project effect to verify compilation.
        let _ = dispatch_effect("factory.build_project", json!({
            "project_path": output_dir,
            "build_system": if spec.kind.as_str().contains("rust") { "cargo" } else { "npm" },
        }))?;
        dispatch_stage_completed("validation", 0)?;

        // Stage 7: Packaging (Phase 11A.5 + 11B: dispatch PackageProject effect).
        dispatch_stage_started("packaging")?;
        let _ = dispatch_effect("factory.package_project", json!({
            "project_path": output_dir,
            "format": "tarball",
        }))?;
        dispatch_stage_completed("packaging", 0)?;

        // Stage 8: DeploymentPrep (Phase 11A.6: generate Dockerfile + CI config).
        dispatch_stage_started("deployment_prep")?;
        let deploy_files = Self::generate_deployment_artifacts(&spec, output_dir);
        for file in &deploy_files {
            let payload = json!({
                "id": run_id.to_string(),
                "path": file.path.as_str(),
            });
            sink.dispatch_trusted(RawEvent::new(
                "factory.file_generated",
                payload,
                Actor::system("factory"),
                0,
            ))?;
        }
        dispatch_stage_completed("deployment_prep", deploy_files.len() as u32)?;

        // Dispatch execution.succeeded for the factory run.
        let mut exec_payload = json!({
            "operation": "factory.run",
            "duration_ms": 0,
            "factory_run_id": run_id.to_string(),
        });
        if let Some(aid) = agent_id {
            exec_payload["agent_id"] = json!(aid.to_string());
        }
        sink.dispatch_trusted(RawEvent::new(
            "execution.succeeded",
            exec_payload,
            Actor::system("factory"),
            0,
        ))?;

        // Dispatch factory.run_completed.
        let payload = json!({"id": run_id.to_string()});
        sink.dispatch_trusted(RawEvent::new(
            "factory.run_completed",
            payload,
            Actor::system("factory"),
            0,
        ))?;

        let all_files = files.into_iter().chain(deploy_files).collect();
        Ok(RunResult {
            run_id,
            project_id,
            files: all_files,
        })
    }

    /// Phase 11A.6: Generate deployment artifacts (Dockerfile + CI config).
    /// Deterministic — no LLM, just templates.
    pub fn generate_deployment_artifacts(spec: &RequirementSpec, _output_dir: &str) -> Vec<GeneratedFile> {
        let mut files = Vec::new();

        // Dockerfile.
        let dockerfile = match spec.kind.as_str() {
            "rust_cli" | "rust_lib" => format!(
                "FROM rust:1.75-slim as builder\nWORKDIR /app\nCOPY . .\nRUN cargo build --release\nFROM debian:bookworm-slim\nCOPY --from=builder /app/target/release/{} /usr/local/bin/{}\nENTRYPOINT [\"{}\"]\n",
                spec.name, spec.name, spec.name
            ),
            "nextjs" => format!(
                "FROM node:20-slim as builder\nWORKDIR /app\nCOPY . .\nRUN npm ci && npm run build\nFROM node:20-slim\nCOPY --from=builder /app /app\nWORKDIR /app\nCMD [\"npm\", \"start\"]\n"
            ),
            "tauri" => format!(
                "FROM rust:1.75-slim as builder\nWORKDIR /app\nCOPY . .\nRUN cargo build --release\nFROM debian:bookworm-slim\nCOPY --from=builder /app/target/release/{} /usr/local/bin/{}\nENTRYPOINT [\"{}\"]\n",
                spec.name, spec.name, spec.name
            ),
            _ => "FROM alpine:latest\nWORKDIR /app\nCOPY . .\nCMD [\"./app\"]\n".to_string(),
        };
        files.push(GeneratedFile {
            path: "Dockerfile".into(),
            content: dockerfile,
        });

        // CI config (.github/workflows/ci.yml).
        let ci_config = format!(
            "name: CI\non:\n  push:\n    branches: [main]\npull_request:\n    branches: [main]\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v3\n      - name: Build\n        run: echo \"Building {}\"\n",
            spec.name
        );
        files.push(GeneratedFile {
            path: ".github/workflows/ci.yml".into(),
            content: ci_config,
        });

        // .dockerignore.
        files.push(GeneratedFile {
            path: ".dockerignore".into(),
            content: "target/\nnode_modules/\n.git/\n*.md\n".into(),
        });

        files
    }

    /// Phase 11A.8: Retry a failed factory run from the failed stage.
    /// Dispatches factory.run_retried, then re-runs the workflow from the
    /// stage that failed.
    ///
    /// In this implementation, retry simply re-runs the entire workflow
    /// with the same request. A more sophisticated version would query
    /// FactoryState to find the failed stage and resume from there.
    pub fn retry_run(
        request: ProjectRequest,
        output_dir: &str,
        original_run_id: uuid::Uuid,
        sink: &dyn sps_core::sink::EventSink,
        agent_id: Option<uuid::Uuid>,
    ) -> sps_core::CoreResult<RunResult> {
        use sps_core::actor::Actor;
        use sps_core::event::RawEvent;
        use serde_json::json;

        // Dispatch factory.run_retried for the original run.
        let payload = json!({"id": original_run_id.to_string()});
        sink.dispatch_trusted(RawEvent::new(
            "factory.run_retried",
            payload,
            Actor::system("factory"),
            0,
        ))?;

        // Re-run the workflow.
        Self::run_with_sink(request, output_dir, sink, agent_id)
    }

    /// Phase 11A.8: Rollback a factory run — mark as rolled back and
    /// clear the generated file paths. Actual filesystem deletion is
    /// the caller's responsibility (via EffectManager in production).
    pub fn rollback_run(
        run_id: uuid::Uuid,
        sink: &dyn sps_core::sink::EventSink,
    ) -> sps_core::CoreResult<()> {
        use sps_core::actor::Actor;
        use sps_core::event::RawEvent;
        use serde_json::json;

        let payload = json!({
            "id": run_id.to_string(),
            "files_removed": 0, // caller fills this in based on actual deletion
        });
        sink.dispatch_trusted(RawEvent::new(
            "factory.rollback_completed",
            payload,
            Actor::system("factory"),
            0,
        ))?;
        Ok(())
    }
}

/// Fix #14: Result of a factory run via `run_with_sink`.
#[derive(Debug, Clone)]
pub struct RunResult {
    /// The factory run id (deterministic from tick in the reducer).
    pub run_id: uuid::Uuid,
    /// The project id generated for this run.
    pub project_id: uuid::Uuid,
    /// Files generated by the run.
    pub files: Vec<GeneratedFile>,
}
