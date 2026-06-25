//! Built-in effect executors: fs, shell, git, search, factory.

pub mod fs;
pub mod shell;
pub mod git;
pub mod search;
pub mod factory;

pub use fs::FsExecutor;
pub use shell::ShellExecutor;
pub use git::GitExecutor;
pub use search::SearchExecutor;
pub use factory::{FactoryExecutor, FactoryExecutorConfig, WriteFileInput, RunTestsInput, BuildProjectInput, PackageProjectInput};
