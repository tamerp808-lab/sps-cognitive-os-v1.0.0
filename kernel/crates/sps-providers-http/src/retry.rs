//! Retry policy with exponential backoff.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Retry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries (0 = no retry).
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Initial backoff duration in ms.
    #[serde(default = "default_initial_backoff_ms")]
    pub initial_backoff_ms: u64,
    /// Maximum backoff duration in ms.
    #[serde(default = "default_max_backoff_ms")]
    pub max_backoff_ms: u64,
    /// Backoff multiplier.
    #[serde(default = "default_multiplier")]
    pub multiplier: f32,
}

fn default_max_retries() -> u32 {
    3
}
fn default_initial_backoff_ms() -> u64 {
    500
}
fn default_max_backoff_ms() -> u64 {
    30_000
}
fn default_multiplier() -> f32 {
    2.0
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_backoff_ms: default_initial_backoff_ms(),
            max_backoff_ms: default_max_backoff_ms(),
            multiplier: default_multiplier(),
        }
    }
}

/// Retry policy — decides whether to retry and how long to wait.
pub struct RetryPolicy {
    config: RetryConfig,
}

impl RetryPolicy {
    /// Create a new policy from config.
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Returns the delay before the next retry attempt (after `attempt`
    /// failures, 0-indexed), or `None` if retries are exhausted.
    pub fn delay_for(&self, attempt: u32) -> Option<Duration> {
        if attempt >= self.config.max_retries {
            return None;
        }
        let base = self.config.initial_backoff_ms as f32;
        let mult = self.config.multiplier.powi(attempt as i32);
        let delay_ms = (base * mult) as u64;
        let delay_ms = delay_ms.min(self.config.max_backoff_ms);
        Some(Duration::from_millis(delay_ms))
    }

    /// Decide whether an error is retryable.
    pub fn is_retryable(err: &anyhow::Error) -> bool {
        // Network errors, timeouts, 5xx → retryable.
        // 4xx (except 429) → not retryable.
        if let Some(req_err) = err.downcast_ref::<reqwest::Error>() {
            if req_err.is_timeout() || req_err.is_connect() {
                return true;
            }
            if req_err.is_status() {
                let code = req_err.status().map(|s| s.as_u16()).unwrap_or(0);
                return code == 429 || code >= 500;
            }
        }
        // Generic errors → assume retryable.
        true
    }

    /// Run an async operation with retry. Returns the final result.
    ///
    /// The operation is called repeatedly until it succeeds or retries
    /// are exhausted. Errors are converted to `anyhow::Error` for the
    /// retryability check.
    pub async fn run<F, T>(&self, mut op: F) -> Result<T, anyhow::Error>
    where
        F: FnMut() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, anyhow::Error>> + Send>>,
        T: Send,
    {
        let mut attempt = 0u32;
        loop {
            match op().await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if !Self::is_retryable(&e) {
                        return Err(e);
                    }
                    match self.delay_for(attempt) {
                        Some(delay) => {
                            tracing::warn!(
                                attempt = attempt + 1,
                                delay_ms = delay.as_millis(),
                                "retrying after error"
                            );
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                        }
                        None => return Err(e),
                    }
                }
            }
        }
    }
}
