use crate::{CoordinationError, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Read-only snapshot of the server-side challenge contract. The fingerprint is deliberately
/// short enough for event metadata but must be lowercase canonical hex; adapters may only run
/// when the challenge id and fingerprint are both known to the control plane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChallengeContract {
    pub schema_version: String,
    pub challenge_id: String,
    pub contract_version: String,
    pub fingerprint: String,
    pub required_submission: String,
    pub status: ContractStatus,
    pub adapter_id: Option<String>,
    pub round_start_ms: Option<i64>,
    pub round_end_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    Known,
    Unknown,
    Unreachable,
}

impl ChallengeContract {
    /// Verifies the canonical fingerprint bytes emitted by the offline contract builder.
    /// This function intentionally accepts canonical JSON bytes rather than arbitrary raw
    /// platform responses so irrelevant page content cannot invalidate the contract.
    pub fn verify_fingerprint_input(
        &self,
        canonical_input: &[u8],
    ) -> Result<(), CoordinationError> {
        let digest = Sha256::digest(canonical_input);
        let fingerprint = digest[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if !self.fingerprint.is_empty() && self.fingerprint == fingerprint {
            Ok(())
        } else {
            Err(invalid(
                "fingerprint does not match the canonical contract input",
            ))
        }
    }

    pub fn validate(
        &self,
        expected_challenge_id: &str,
        now_ms: i64,
    ) -> Result<(), CoordinationError> {
        if self.schema_version != SCHEMA_VERSION
            || self.challenge_id != expected_challenge_id
            || self.challenge_id.trim().is_empty()
            || self.contract_version.trim().is_empty()
            || self.required_submission.trim().is_empty()
            || self.fingerprint.len() != 16
            || self.fingerprint != self.fingerprint.to_ascii_lowercase()
            || !self
                .fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid(
                "contract identity, version, submission schema, and fingerprint are required",
            ));
        }
        if let (Some(start), Some(end)) = (self.round_start_ms, self.round_end_ms) {
            if start < 0 || end <= start || now_ms < start {
                return Err(invalid("contract round window is invalid or not yet open"));
            }
        }
        if self.round_end_ms.is_some_and(|end| now_ms >= end) {
            return Err(invalid("contract round window has closed"));
        }
        match self.status {
            ContractStatus::Known if self.adapter_id.as_deref().is_none_or(str::is_empty) => {
                Err(invalid("known contract requires an exact adapter id"))
            }
            ContractStatus::Unknown if self.adapter_id.is_some() => {
                Err(invalid("unknown contract cannot claim an adapter"))
            }
            ContractStatus::Unreachable if self.adapter_id.is_some() => {
                Err(invalid("unreachable contract cannot claim an adapter"))
            }
            _ => Ok(()),
        }
    }

    /// Formal submission/closure admission. `Unknown` and `Unreachable` remain fail-closed;
    /// callers may only use generic local safety checks until a fresh known contract is recorded.
    pub fn formal_admission(
        &self,
        expected_challenge_id: &str,
        now_ms: i64,
    ) -> Result<(), CoordinationError> {
        self.validate(expected_challenge_id, now_ms)?;
        if self.status != ContractStatus::Known {
            return Err(invalid(
                "formal admission requires a known, fingerprint-matched contract",
            ));
        }
        Ok(())
    }
}

fn invalid(message: &str) -> CoordinationError {
    CoordinationError::InvalidDecision(format!("challenge contract is invalid: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(status: ContractStatus) -> ChallengeContract {
        ChallengeContract {
            schema_version: SCHEMA_VERSION.into(),
            challenge_id: "challenge-1".into(),
            contract_version: "v1".into(),
            fingerprint: "0123456789abcdef".into(),
            required_submission: "json".into(),
            status,
            adapter_id: (status == ContractStatus::Known).then(|| "adapter-v1".into()),
            round_start_ms: Some(100),
            round_end_ms: Some(500),
        }
    }

    #[test]
    fn unknown_contract_is_not_formally_admissible() {
        assert!(
            contract(ContractStatus::Unknown)
                .validate("challenge-1", 200)
                .is_ok()
        );
        assert!(
            contract(ContractStatus::Unknown)
                .formal_admission("challenge-1", 200)
                .is_err()
        );
    }

    #[test]
    fn known_contract_requires_exact_identity_and_open_window() {
        assert!(
            contract(ContractStatus::Known)
                .formal_admission("challenge-1", 200)
                .is_ok()
        );
        assert!(
            contract(ContractStatus::Known)
                .formal_admission("other", 200)
                .is_err()
        );
        assert!(
            contract(ContractStatus::Known)
                .formal_admission("challenge-1", 500)
                .is_err()
        );
    }

    #[test]
    fn fingerprint_matches_canonical_input_used_by_the_offline_builder() {
        let mut candidate = contract(ContractStatus::Known);
        candidate.fingerprint = "0000000000000000".into();
        assert!(candidate.verify_fingerprint_input(b"{}").is_err());
        candidate.fingerprint = "d72ab8e1a3c4d9f2".into();
        assert!(candidate.verify_fingerprint_input(b"{}").is_err());

        let digest = Sha256::digest(br#"{"challenge_id":"challenge-1"}"#);
        let fingerprint = digest[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        candidate.fingerprint = fingerprint;
        assert!(
            candidate
                .verify_fingerprint_input(br#"{"challenge_id":"challenge-1"}"#)
                .is_ok()
        );
    }
}
