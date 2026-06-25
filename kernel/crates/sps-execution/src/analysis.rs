//! Code analyzer.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// Analysis of a single file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileAnalysis {
    /// File path.
    pub path: SmolStr,
    /// Lines of code.
    pub lines: u32,
    /// Detected language.
    pub language: SmolStr,
    /// Number of imports.
    pub imports: u32,
    /// Number of functions.
    pub functions: u32,
    /// Number of task comments.
    pub todos: u32,
}

/// Analysis of a codebase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeAnalysis {
    /// All files analyzed.
    pub files: Vec<FileAnalysis>,
    /// Total lines.
    pub total_lines: u32,
    /// Total functions.
    pub total_functions: u32,
    /// Language breakdown.
    pub languages: std::collections::BTreeMap<String, u32>,
}

/// Analyze source code files.
pub struct CodeAnalyzer;

impl CodeAnalyzer {
    /// Analyze a single file's content.
    pub fn analyze_file(path: &str, content: &str) -> FileAnalysis {
        let lines = content.lines().count() as u32;
        let language = detect_language(path);
        let imports = count_imports(content, &language);
        let functions = count_functions(content, &language);
        let todos = content.matches("TASK").count() as u32;
        FileAnalysis {
            path: path.into(),
            lines,
            language,
            imports,
            functions,
            todos,
        }
    }

    /// Analyze multiple files.
    pub fn analyze(files: &[(String, String)]) -> CodeAnalysis {
        let mut analyses = Vec::new();
        let mut total_lines = 0u32;
        let mut total_functions = 0u32;
        let mut languages: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();

        for (path, content) in files {
            let a = Self::analyze_file(path, content);
            total_lines += a.lines;
            total_functions += a.functions;
            *languages.entry(a.language.to_string()).or_default() += 1;
            analyses.push(a);
        }

        CodeAnalysis {
            files: analyses,
            total_lines,
            total_functions,
            languages,
        }
    }
}

fn detect_language(path: &str) -> SmolStr {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust".into(),
        "ts" | "tsx" => "typescript".into(),
        "js" | "jsx" => "javascript".into(),
        "py" => "python".into(),
        "go" => "go".into(),
        "java" => "java".into(),
        "c" | "h" => "c".into(),
        "cpp" | "cc" | "hpp" => "cpp".into(),
        "md" => "markdown".into(),
        "json" => "json".into(),
        "toml" => "toml".into(),
        _ => "unknown".into(),
    }
}

fn count_imports(content: &str, language: &str) -> u32 {
    match language {
        "rust" => content.lines().filter(|l| l.trim_start().starts_with("use ")).count() as u32,
        "typescript" | "javascript" => content
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                t.starts_with("import ") || t.starts_with("require(")
            })
            .count() as u32,
        "python" => content
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                t.starts_with("import ") || t.starts_with("from ")
            })
            .count() as u32,
        _ => 0,
    }
}

fn count_functions(content: &str, language: &str) -> u32 {
    match language {
        "rust" => content.lines().filter(|l| l.contains("fn ")).count() as u32,
        "typescript" | "javascript" => {
            content.matches("function ").count() as u32 + content.matches("=>").count() as u32
        }
        "python" => content.lines().filter(|l| l.trim_start().starts_with("def ")).count() as u32,
        _ => 0,
    }
}
