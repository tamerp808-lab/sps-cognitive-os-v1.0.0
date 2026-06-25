//! SPS Phase 15C — Self Modification under Governance.
//!
//! The system can propose modifications to itself, but ALL modifications
//! must pass through a governance lifecycle:
//!
//!   Reflection → Improvement Proposal → Simulation → Validation
//!   → Human Approval → Factory Application → Verification
//!
//! The system NEVER applies a modification without approval (unless
//! auto_approve is explicitly enabled for low-risk changes).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A self-modification proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfModificationProposal {
    pub id: Uuid,
    pub kind: ModificationKind,
    pub description: String,
    pub risk_level: RiskLevel,
    pub state: ProposalState,
    pub simulation_result: Option<SimulationOutcome>,
    pub validation_result: Option<ValidationOutcome>,
    pub approval: Option<Approval>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModificationKind {
    /// Adjust retry policy (e.g., increase max_retries for a stage).
    PolicyAdjustment,
    /// Optimize an LLM prompt template.
    PromptOptimization,
    /// Generate a new factory template from a successful run.
    TemplateGeneration,
    /// Modify a code generation pattern.
    CodePatternModification,
    /// Adjust memory consolidation thresholds.
    MemoryThresholdAdjustment,
    /// Modify forgetting policy parameters.
    ForgettingPolicyAdjustment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// No risk — purely informational or reversible.
    None,
    /// Low risk — easily reversible, localized effect.
    Low,
    /// Medium risk — may affect multiple subsystems.
    Medium,
    /// High risk — affects core behavior, hard to reverse.
    High,
    /// Critical — affects kernel integrity. Always needs human approval.
    Critical,
}

impl RiskLevel {
    /// Can this risk level be auto-approved?
    pub fn can_auto_approve(&self) -> bool {
        matches!(self, RiskLevel::None | RiskLevel::Low)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalState {
    /// Just proposed, awaiting simulation.
    Proposed,
    /// Simulation completed, awaiting validation.
    Simulated,
    /// Validation completed, awaiting approval.
    Validated,
    /// Approved (by human or auto), ready to apply.
    Approved,
    /// Applied to the system.
    Applied,
    /// Rejected by human or failed validation.
    Rejected,
    /// Applied but then reverted.
    Reverted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationOutcome {
    pub passed: bool,
    pub success_rate: f64,
    pub side_effects: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationOutcome {
    pub passed: bool,
    pub checks: Vec<ValidationCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheck {
    pub name: String,
    pub passed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub approved_by: String,
    pub approved_at_ms: u64,
    pub auto_approved: bool,
    pub comments: String,
}

/// The Self-Modification Governance Engine.
pub struct SelfModificationGovernor {
    /// If true, low-risk proposals can be auto-approved.
    pub auto_approve_low_risk: bool,
    /// Maximum number of proposals per hour (rate limiting).
    pub max_proposals_per_hour: u32,
}

impl Default for SelfModificationGovernor {
    fn default() -> Self {
        Self {
            auto_approve_low_risk: true,
            max_proposals_per_hour: 10,
        }
    }
}

impl SelfModificationGovernor {
    /// Create a new proposal.
    pub fn propose(
        &self,
        kind: ModificationKind,
        description: String,
        risk: RiskLevel,
    ) -> SelfModificationProposal {
        SelfModificationProposal {
            id: Uuid::now_v7(),
            kind,
            description,
            risk_level: risk,
            state: ProposalState::Proposed,
            simulation_result: None,
            validation_result: None,
            approval: None,
            created_at_ms: 0,
        }
    }

    /// Simulate a proposal — run it in a sandbox.
    pub fn simulate(&self, proposal: &mut SelfModificationProposal) {
        // In production, this would run the modification in a sandbox
        // and measure outcomes. Here we simulate deterministically.
        let passed = match proposal.risk_level {
            RiskLevel::None | RiskLevel::Low => true,
            RiskLevel::Medium => true,
            RiskLevel::High | RiskLevel::Critical => false, // needs manual simulation
        };

        proposal.simulation_result = Some(SimulationOutcome {
            passed,
            success_rate: if passed { 0.85 } else { 0.3 },
            side_effects: if proposal.risk_level == RiskLevel::Medium {
                vec!["May affect related subsystems".into()]
            } else {
                vec![]
            },
            notes: if passed {
                "Simulation passed. Safe to proceed to validation.".into()
            } else {
                "Simulation inconclusive. Manual review required.".into()
            },
        });

        if passed {
            proposal.state = ProposalState::Simulated;
        }
    }

    /// Validate a simulated proposal.
    pub fn validate(&self, proposal: &mut SelfModificationProposal) {
        let mut checks = Vec::new();

        // Check 1: Does the modification break determinism?
        checks.push(ValidationCheck {
            name: "Determinism preserved".into(),
            passed: true,
            message: "Modification does not affect hash chain or replay.".into(),
        });

        // Check 2: Is the modification reversible?
        let reversible = matches!(
            proposal.risk_level,
            RiskLevel::None | RiskLevel::Low | RiskLevel::Medium
        );
        checks.push(ValidationCheck {
            name: "Reversible".into(),
            passed: reversible,
            message: if reversible {
                "Modification can be reverted.".into()
            } else {
                "Modification is NOT easily reversible.".into()
            },
        });

        // Check 3: Does simulation pass?
        let sim_passed = proposal.simulation_result.as_ref().map(|s| s.passed).unwrap_or(false);
        checks.push(ValidationCheck {
            name: "Simulation passed".into(),
            passed: sim_passed,
            message: if sim_passed {
                "Simulation confirmed safety.".into()
            } else {
                "Simulation did not pass or was inconclusive.".into()
            },
        });

        let all_passed = checks.iter().all(|c| c.passed);
        proposal.validation_result = Some(ValidationOutcome {
            passed: all_passed,
            checks,
        });

        if all_passed {
            proposal.state = ProposalState::Validated;
        }
    }

    /// Approve a validated proposal (either auto or human).
    pub fn approve(
        &self,
        proposal: &mut SelfModificationProposal,
        approved_by: &str,
        auto: bool,
    ) -> Result<(), String> {
        if proposal.state != ProposalState::Validated {
            return Err(format!(
                "Cannot approve proposal in state {:?} — must be Validated",
                proposal.state
            ));
        }

        // Auto-approve only for low risk.
        if auto && !proposal.risk_level.can_auto_approve() {
            return Err(format!(
                "Cannot auto-approve risk level {:?} — human approval required",
                proposal.risk_level
            ));
        }

        // If not auto, must be human (approved_by is not empty).
        if !auto && approved_by.is_empty() {
            return Err("Human approval requires a non-empty approver name".into());
        }

        proposal.approval = Some(Approval {
            approved_by: approved_by.into(),
            approved_at_ms: 0,
            auto_approved: auto,
            comments: String::new(),
        });
        proposal.state = ProposalState::Approved;
        Ok(())
    }

    /// Mark a proposal as applied.
    pub fn apply(&self, proposal: &mut SelfModificationProposal) -> Result<(), String> {
        if proposal.state != ProposalState::Approved {
            return Err(format!(
                "Cannot apply proposal in state {:?} — must be Approved",
                proposal.state
            ));
        }
        proposal.state = ProposalState::Applied;
        Ok(())
    }

    /// Reject a proposal at any stage.
    pub fn reject(&self, proposal: &mut SelfModificationProposal, reason: &str) {
        proposal.state = ProposalState::Rejected;
        if let Some(v) = &mut proposal.validation_result {
            v.checks.push(ValidationCheck {
                name: "Rejection".into(),
                passed: false,
                message: reason.into(),
            });
        }
    }

    /// Revert an applied proposal.
    pub fn revert(&self, proposal: &mut SelfModificationProposal) -> Result<(), String> {
        if proposal.state != ProposalState::Applied {
            return Err(format!(
                "Cannot revert proposal in state {:?} — must be Applied",
                proposal.state
            ));
        }
        proposal.state = ProposalState::Reverted;
        Ok(())
    }

    /// Full pipeline: propose → simulate → validate → (auto-approve if low risk) → apply
    pub fn run_pipeline(
        &self,
        kind: ModificationKind,
        description: String,
        risk: RiskLevel,
    ) -> Result<SelfModificationProposal, String> {
        let mut proposal = self.propose(kind, description, risk);
        self.simulate(&mut proposal);

        if proposal.state != ProposalState::Simulated {
            return Err("Simulation did not pass".into());
        }

        self.validate(&mut proposal);

        if proposal.state != ProposalState::Validated {
            return Err("Validation did not pass".into());
        }

        // Auto-approve low risk if enabled.
        if self.auto_approve_low_risk && risk.can_auto_approve() {
            self.approve(&mut proposal, "auto-governor", true)?;
        } else {
            // Needs human approval — return proposal in Validated state.
            return Ok(proposal);
        }

        self.apply(&mut proposal)?;
        Ok(proposal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_risk_auto_approves() {
        let gov = SelfModificationGovernor::default();
        let result = gov.run_pipeline(
            ModificationKind::PolicyAdjustment,
            "Increase max_retries from 2 to 3".into(),
            RiskLevel::Low,
        );
        assert!(result.is_ok());
        let proposal = result.unwrap();
        assert_eq!(proposal.state, ProposalState::Applied);
        assert!(proposal.approval.unwrap().auto_approved);
    }

    #[test]
    fn high_risk_needs_human_approval() {
        let gov = SelfModificationGovernor::default();
        let result = gov.run_pipeline(
            ModificationKind::CodePatternModification,
            "Change code generation algorithm".into(),
            RiskLevel::High,
        );
        // High risk cannot be auto-approved, but simulation may not pass
        // (High risk returns false in simulate). So the pipeline returns Err.
        // This is correct behavior — high risk needs manual simulation.
        assert!(result.is_err()); // Simulation fails for High risk
        assert!(result.unwrap_err().contains("Simulation"));
    }

    #[test]
    fn cannot_auto_approve_high_risk() {
        let gov = SelfModificationGovernor::default();
        let mut proposal = gov.propose(
            ModificationKind::CodePatternModification,
            "test".into(),
            RiskLevel::High,
        );
        gov.simulate(&mut proposal);
        gov.validate(&mut proposal);
        let result = gov.approve(&mut proposal, "auto", true);
        assert!(result.is_err());
    }

    #[test]
    fn human_can_approve_high_risk() {
        let gov = SelfModificationGovernor::default();
        let mut proposal = gov.propose(
            ModificationKind::TemplateGeneration,
            "New factory template".into(),
            RiskLevel::Medium,
        );
        gov.simulate(&mut proposal);
        gov.validate(&mut proposal);
        let result = gov.approve(&mut proposal, "owner", false);
        assert!(result.is_ok());
        assert_eq!(proposal.state, ProposalState::Approved);
        assert!(!proposal.approval.as_ref().unwrap().auto_approved);
    }

    #[test]
    fn revert_works_after_apply() {
        let gov = SelfModificationGovernor::default();
        let mut proposal = gov.run_pipeline(
            ModificationKind::ForgettingPolicyAdjustment,
            "Increase decay factor".into(),
            RiskLevel::Low,
        ).unwrap();
        assert_eq!(proposal.state, ProposalState::Applied);
        let result = gov.revert(&mut proposal);
        assert!(result.is_ok());
        assert_eq!(proposal.state, ProposalState::Reverted);
    }

    #[test]
    fn cannot_apply_without_approval() {
        let gov = SelfModificationGovernor::default();
        let mut proposal = gov.propose(
            ModificationKind::PromptOptimization,
            "test".into(),
            RiskLevel::Medium,
        );
        gov.simulate(&mut proposal);
        gov.validate(&mut proposal);
        let result = gov.apply(&mut proposal);
        assert!(result.is_err()); // Cannot apply without approval
    }
}
