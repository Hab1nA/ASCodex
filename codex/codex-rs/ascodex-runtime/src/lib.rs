//! Trusted loading and compact rendering of Chief-issued ASCodex StageBrief records.
//!
//! This crate deliberately has no model, network, or executor dependency. It resolves an immutable
//! brief from the Guard ledger and turns it into a bounded developer-context card only after
//! revalidating every referenced file beneath a canonical workspace root. JSON bundles remain a
//! compatibility/import format and are not authoritative for solver admission.

use codex_ascodex_coordination::{Role, SkillRef, StageBrief, WorkspaceAcl, WorkspaceAclError};
use codex_solver_guard::{Ledger, StageBriefLedgerTarget};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const BUNDLE_SCHEMA_VERSION: &str = "ascodex-stage-brief-bundle/v1";

#[derive(Debug, Error)]
pub enum StageBriefRuntimeError {
    #[error("stage brief bundle path must be absolute")]
    BundlePathNotAbsolute,
    #[error("stage brief bundle is invalid: {0}")]
    InvalidBundle(String),
    #[error("stage brief bundle I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("stage brief bundle JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("role workspace ACL is invalid: {0}")]
    WorkspaceAcl(#[from] WorkspaceAclError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageBriefBundle {
    pub schema_version: String,
    pub workspace_root: PathBuf,
    pub capability_map_path: String,
    pub briefs: Vec<StageBrief>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageBriefTarget<'a> {
    pub campaign_id: &'a str,
    pub challenge_id: &'a str,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedStageBriefTarget<'a> {
    pub cycle_id: &'a str,
    pub campaign_id: &'a str,
    pub challenge_id: &'a str,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedStageBrief {
    pub brief_id: String,
    pub cycle_id: Option<String>,
    pub cycle_event_version: Option<u64>,
    /// Role authorized by the signed StageBrief.
    pub role: Role,
    /// Canonical project policy root used only to verify the capability map and signed skills.
    pub policy_root: PathBuf,
    /// Canonical role workspace root signed by the StageBrief. Core derives filesystem write
    /// authority from this root, never from `policy_root` or inherited parent roots.
    pub workspace_root: PathBuf,
    /// Whether the brief was issued for an independent clean-room role.
    pub clean_room: bool,
    /// Signed skill references selected by the Chief. Core may grant read access
    /// to these exact files without widening the role workspace.
    pub skills: Vec<SkillRef>,
    pub rendered: String,
}

/// Loads a JSON bundle from an administrator-provided absolute path and returns exactly one
/// verified StageBrief for the specified child. Multiple matches fail closed.
pub fn load_and_render_brief(
    bundle_path: &Path,
    target: StageBriefTarget<'_>,
    now_ms: i64,
) -> Result<VerifiedStageBrief, StageBriefRuntimeError> {
    if !bundle_path.is_absolute() {
        return Err(StageBriefRuntimeError::BundlePathNotAbsolute);
    }
    let raw = fs::read_to_string(bundle_path)?;
    let bundle: StageBriefBundle = serde_json::from_str(&raw)?;
    if bundle.schema_version != BUNDLE_SCHEMA_VERSION {
        return Err(invalid("unsupported bundle schema version"));
    }
    let matches = bundle
        .briefs
        .iter()
        .filter(|brief| {
            brief.campaign_id == target.campaign_id
                && brief.challenge_id == target.challenge_id
                && brief.target_role == target.role
        })
        .collect::<Vec<_>>();
    let [brief] = matches.as_slice() else {
        return Err(invalid("bundle must contain exactly one matching brief"));
    };

    verify_and_render_brief(
        brief,
        &bundle.workspace_root,
        &bundle.capability_map_path,
        target,
        now_ms,
    )
}

/// Loads the one immutable StageBrief that a Chief recorded in the ledger, then independently
/// verifies every local file it references before exposing bounded developer context.
pub async fn load_and_render_issued_brief(
    ledger_path: &Path,
    target: IssuedStageBriefTarget<'_>,
    now_ms: i64,
) -> Result<VerifiedStageBrief, StageBriefRuntimeError> {
    if !ledger_path.is_absolute() {
        return Err(StageBriefRuntimeError::BundlePathNotAbsolute);
    }
    let ledger = Ledger::open_file(ledger_path)
        .await
        .map_err(|error| invalid(&format!("cannot open issued stage brief ledger: {error}")))?;
    let persisted = match ledger
        .load_stage_brief_issuance(
            &StageBriefLedgerTarget {
                cycle_id: target.cycle_id,
                campaign_id: target.campaign_id,
                challenge_id: target.challenge_id,
                role: target.role,
            },
            now_ms,
        )
        .await
    {
        Ok(persisted) => persisted,
        Err(error) => {
            ledger.close().await;
            return Err(invalid(&format!(
                "issued stage brief is unavailable: {error}"
            )));
        }
    };
    ledger.close().await;
    let mut verified = verify_and_render_brief(
        &persisted.stage_brief,
        &persisted.workspace_root,
        &persisted.capability_map_path,
        StageBriefTarget {
            campaign_id: target.campaign_id,
            challenge_id: target.challenge_id,
            role: target.role,
        },
        now_ms,
    )?;
    verified.cycle_id = Some(persisted.cycle_id);
    verified.cycle_event_version = Some(persisted.cycle_event_version);
    Ok(verified)
}

pub fn verify_and_render_brief(
    brief: &StageBrief,
    workspace_root: &Path,
    capability_map_path: &str,
    target: StageBriefTarget<'_>,
    now_ms: i64,
) -> Result<VerifiedStageBrief, StageBriefRuntimeError> {
    if brief.campaign_id != target.campaign_id
        || brief.challenge_id != target.challenge_id
        || brief.target_role != target.role
    {
        return Err(invalid("stage brief does not match its worker target"));
    }
    let policy_root = fs::canonicalize(workspace_root)?;
    if !policy_root.is_dir() {
        return Err(invalid("policy root is not a directory"));
    }
    let challenge_workspace_root = fs::canonicalize(&brief.challenge_workspace_root)?;
    if !challenge_workspace_root.is_dir() || policy_root.starts_with(&challenge_workspace_root) {
        return Err(invalid(
            "challenge workspace must be a directory and cannot contain the policy root",
        ));
    }
    let capability_map = resolve_relative_file(&policy_root, capability_map_path)?;
    brief
        .validate(now_ms)
        .map_err(|error| invalid(&error.to_string()))?;
    if digest_file(&capability_map)? != brief.capability_map_sha256.to_ascii_lowercase() {
        return Err(invalid("capability map digest does not match the brief"));
    }
    for skill in &brief.skills {
        let path = resolve_relative_file(&policy_root, &skill.source_path)?;
        if digest_file(&path)? != skill.sha256.to_ascii_lowercase() {
            return Err(invalid(&format!(
                "skill digest does not match: {}",
                skill.name
            )));
        }
    }
    Ok(VerifiedStageBrief {
        brief_id: brief.brief_id.clone(),
        cycle_id: None,
        cycle_event_version: None,
        policy_root,
        workspace_root: challenge_workspace_root,
        role: brief.target_role,
        clean_room: brief.clean_room,
        skills: brief.skills.clone(),
        rendered: render_brief(brief)?,
    })
}

/// Derive and validate the role's filesystem visibility from a verified, Chief-issued brief.
///
/// Core should call this before constructing a child environment, then translate the returned
/// explicit roots into its managed filesystem policy. Inherited roots and process environment
/// selectors must not be used as a substitute for this check.
pub fn role_workspace_acl(
    brief: &VerifiedStageBrief,
    parent_roots: &[PathBuf],
) -> Result<WorkspaceAcl, StageBriefRuntimeError> {
    let mut acl = WorkspaceAcl::for_role(&brief.workspace_root, brief.role)?;
    if brief.clean_room != (brief.role == Role::RedTeam) {
        return Err(StageBriefRuntimeError::WorkspaceAcl(
            WorkspaceAclError::CleanRoomOverlapsParent,
        ));
    }
    acl.validate_clean_room_isolation(parent_roots)?;
    // Selected skills are signed inputs to the brief. Add only those exact files to the
    // readable set for non-solver roles; never widen the ACL to the whole `.agents` tree.
    for skill in &brief.skills {
        let path = resolve_relative_file(&brief.policy_root, &skill.source_path)?;
        if !acl.readable_roots.iter().any(|root| root == &path) {
            acl.readable_roots.push(path);
        }
    }
    Ok(acl)
}

fn render_brief(brief: &StageBrief) -> Result<String, StageBriefRuntimeError> {
    let mut text = format!(
        "Verified ASCodex research brief. Campaign: {}. Challenge: {}. Stage: {:?}. \
         Read the selected skill files before acting; this brief grants no tool, identity, lease, \
         network, or submission permission.\n",
        brief.campaign_id, brief.challenge_id, brief.stage
    );
    for skill in &brief.skills {
        text.push_str(&format!(
            "- {}: {} ({})\n",
            skill.name, skill.source_path, skill.selection_reason
        ));
    }
    text.push_str(&format!("Selection: {}", brief.selection_reason));
    if text.len() > usize::try_from(brief.max_bytes).expect("u32 fits usize")
        || text.len() > usize::try_from(brief.estimated_bytes).expect("u32 fits usize")
    {
        return Err(invalid(
            "rendered brief exceeds its declared context budget",
        ));
    }
    Ok(text)
}

fn resolve_relative_file(root: &Path, path: &str) -> Result<PathBuf, StageBriefRuntimeError> {
    let relative = Path::new(path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(invalid(
            "bundle path must be workspace-relative without parent traversal",
        ));
    }
    let candidate = root.join(relative);
    let canonical = fs::canonicalize(&candidate)?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(invalid("bundle path escapes workspace or is not a file"));
    }
    Ok(canonical)
}

fn digest_file(path: &Path) -> Result<String, StageBriefRuntimeError> {
    let bytes = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn invalid(message: &str) -> StageBriefRuntimeError {
    StageBriefRuntimeError::InvalidBundle(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_ascodex_coordination::{
        Action, ActorContext, ChiefDirective, CycleDirective, CycleOutcome, EvidenceRef,
        ExperimentPlan, Lease, MAX_STAGE_BRIEF_BYTES, OodaCycleRecord, OodaPhase, ReportStatus,
        ResearchCycleRecord, ResearchStage, SCHEMA_VERSION, SkillRef, WorkerReport,
    };
    use codex_solver_guard::CoordinationEventRecord;
    use std::collections::BTreeSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn digest(value: &[u8]) -> String {
        format!("{:x}", Sha256::digest(value))
    }

    fn fixture() -> (PathBuf, PathBuf, StageBriefTarget<'static>) {
        let root = std::env::temp_dir().join(format!(
            "ascodex-stage-brief-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let skill_dir = root.join(".agents/skills/bohrium-bohr");
        fs::create_dir_all(&skill_dir).unwrap();
        let skill_bytes = b"trusted staged knowledge";
        fs::write(skill_dir.join("SKILL.md"), skill_bytes).unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        let capability_bytes = b"capability-map";
        fs::write(root.join("config/capability-map.md"), capability_bytes).unwrap();
        let challenge_root = root.join("work/challenge-1");
        fs::create_dir_all(&challenge_root).unwrap();
        let brief = StageBrief {
            schema_version: SCHEMA_VERSION.into(),
            brief_id: "brief-1".into(),
            campaign_id: "campaign-1".into(),
            challenge_id: "challenge-1".into(),
            challenge_workspace_root: challenge_root,
            stage: ResearchStage::CloudCompute,
            target_role: Role::Solver,
            generated_at_ms: 100,
            expires_at_ms: 200,
            max_bytes: MAX_STAGE_BRIEF_BYTES,
            estimated_bytes: 800,
            skills: vec![SkillRef {
                name: "bohrium-bohr".into(),
                source_path: ".agents/skills/bohrium-bohr/SKILL.md".into(),
                sha256: digest(skill_bytes),
                selection_reason: "cloud compute stage".into(),
            }],
            selection_reason: "bounded test brief".into(),
            capability_map_sha256: digest(capability_bytes),
            clean_room: false,
        };
        let bundle = StageBriefBundle {
            schema_version: BUNDLE_SCHEMA_VERSION.into(),
            workspace_root: root.clone(),
            capability_map_path: "config/capability-map.md".into(),
            briefs: vec![brief],
        };
        let bundle_path = root.join("stage-briefs.json");
        fs::write(&bundle_path, serde_json::to_vec(&bundle).unwrap()).unwrap();
        (
            root,
            bundle_path,
            StageBriefTarget {
                campaign_id: "campaign-1",
                challenge_id: "challenge-1",
                role: Role::Solver,
            },
        )
    }

    #[test]
    fn role_workspace_acl_requires_disjoint_clean_room_parent() {
        let root = std::env::temp_dir().join(format!(
            "ascodex-workspace-acl-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("knowledge")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("knowledge/problem.md"), b"problem").unwrap();
        fs::write(root.join("src/input.json"), b"{}").unwrap();
        let brief = VerifiedStageBrief {
            brief_id: "brief-clean".into(),
            cycle_id: Some("cycle-1".into()),
            cycle_event_version: Some(1),
            role: Role::RedTeam,
            policy_root: fs::canonicalize(&root).unwrap(),
            workspace_root: fs::canonicalize(&root).unwrap(),
            clean_room: true,
            skills: Vec::new(),
            rendered: "verified".into(),
        };
        let parent = root.with_file_name(format!(
            "{}-solver",
            root.file_name().unwrap().to_string_lossy()
        ));
        fs::create_dir_all(&parent).unwrap();
        let acl = role_workspace_acl(&brief, &[parent.clone()]).unwrap();
        assert!(acl.can_read(&root.join("src/input.json")));
        assert!(!acl.can_read(&root.join("outputs/report.json")));

        assert!(role_workspace_acl(&brief, &[root.clone()]).is_err());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(parent);
    }

    fn evidence(kind: &str, marker: char) -> EvidenceRef {
        EvidenceRef {
            kind: kind.to_string(),
            path: format!("evidence/{kind}-{marker}.json"),
            sha256: Some(marker.to_string().repeat(64)),
        }
    }

    fn issued_cycle(brief: StageBrief) -> ResearchCycleRecord {
        ResearchCycleRecord {
            schema_version: SCHEMA_VERSION.to_string(),
            cycle_id: "cycle-1".to_string(),
            campaign_id: "campaign-1".to_string(),
            challenge_id: "challenge-1".to_string(),
            expected_state_version: 0,
            deadline_ms: 190,
            verifier_spec_sha256: "a".repeat(64),
            baseline_sha256: "b".repeat(64),
            stage_briefs: vec![brief],
            experiment_plan: Some(ExperimentPlan {
                schema_version: SCHEMA_VERSION.to_string(),
                challenge_id: "challenge-1".to_string(),
                axis: "one field".to_string(),
                changed_fields: vec!["field-a".to_string()],
                coupled_group: None,
                hypothesis: "the normalized field matches the verifier".to_string(),
                expected_response: "one typed response".to_string(),
                decision_criterion: "compare the isolated score component".to_string(),
                parent_attempt_id: None,
            }),
            worker_report: Some(WorkerReport {
                schema_version: SCHEMA_VERSION.to_string(),
                role: Role::Solver,
                status: ReportStatus::Blocked,
                challenge_id: "challenge-1".to_string(),
                identity: None,
                attempt_id: None,
                harbor_reward: None,
                trace_score: None,
                judge_summary: None,
                evidence: vec![],
            }),
            observation: None,
            contract: None,
            facts: vec!["The local verifier contract is available.".to_string()],
            inferences: vec!["A bounded dispatch is warranted.".to_string()],
            outcome: CycleOutcome::Blocked,
            directive: ChiefDirective::Replan,
            closure_evidence: None,
            quota_cost: 0.0,
            evidence: vec![
                evidence("verifier", 'a'),
                evidence("baseline", 'b'),
                evidence("stage_brief", 'c'),
            ],
            ooda: OodaCycleRecord {
                schema_version: SCHEMA_VERSION.to_string(),
                cycle_id: "cycle-1".to_string(),
                campaign_id: "campaign-1".to_string(),
                challenge_id: "challenge-1".to_string(),
                phase: OodaPhase::Decide,
                actor_role: Role::Chief,
                directive: CycleDirective::Replan,
                rationale: "Issue one bounded worker dispatch.".to_string(),
                expected_state_version: 0,
                deadline_ms: 190,
                stuck_triggers: vec![],
                evidence: vec![evidence("baseline", 'b')],
            },
        }
    }

    fn chief_context() -> ActorContext {
        ActorContext {
            agent_id: "chief-1".to_string(),
            session_id: "session-1".to_string(),
            thread_id: "thread-1".to_string(),
            role: Role::Chief,
            campaign_id: "campaign-1".to_string(),
            challenge_id: "challenge-1".to_string(),
            lease: Lease {
                lease_id: "chief-lease-1".to_string(),
                campaign_id: "campaign-1".to_string(),
                challenge_id: "challenge-1".to_string(),
                owner_agent_id: "chief-1".to_string(),
                role: Role::Chief,
                issued_at_ms: 100,
                expires_at_ms: 1_000,
                epoch: 1,
                allowed_actions: BTreeSet::from([Action::Decide, Action::SpawnChild]),
                authorized_identity_classes: BTreeSet::from(["chief-primary".to_string()]),
                operator_id: "operator-1".to_string(),
                pool_epoch: 1,
                registration_allowed: false,
            },
        }
    }

    #[test]
    fn verifies_and_renders_a_bounded_brief() {
        let (root, bundle, target) = fixture();
        let result = load_and_render_brief(&bundle, target, 150).unwrap();
        assert_eq!(result.brief_id, "brief-1");
        assert_eq!(result.policy_root, fs::canonicalize(&root).unwrap());
        assert_eq!(
            result.workspace_root,
            fs::canonicalize(root.join("work/challenge-1")).unwrap()
        );
        assert!(result.rendered.contains("bohrium-bohr"));
        assert!(result.rendered.contains("grants no tool"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_tampered_skill_wrong_target_and_expired_brief() {
        let (root, bundle, target) = fixture();
        fs::write(
            root.join(".agents/skills/bohrium-bohr/SKILL.md"),
            "tampered",
        )
        .unwrap();
        assert!(load_and_render_brief(&bundle, target.clone(), 150).is_err());
        fs::remove_dir_all(&root).unwrap();

        let (root, bundle, mut target) = fixture();
        target.role = Role::RedTeam;
        assert!(load_and_render_brief(&bundle, target, 150).is_err());
        assert!(
            load_and_render_brief(
                &bundle,
                StageBriefTarget {
                    campaign_id: "campaign-1",
                    challenge_id: "challenge-1",
                    role: Role::Solver
                },
                200
            )
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn issued_brief_runtime_canary_fails_closed_after_cycle_revoke() {
        let (root, bundle_path, _) = fixture();
        let bundle: StageBriefBundle =
            serde_json::from_slice(&fs::read(&bundle_path).unwrap()).unwrap();
        let cycle = issued_cycle(bundle.briefs[0].clone());
        let chief = chief_context();
        let ledger_path = root.join("guard.sqlite");
        let ledger = Ledger::connect_file(&ledger_path).await.unwrap();
        ledger.provision_actor_context(&chief, 150).await.unwrap();
        let cycle_payload = serde_json::to_string(&cycle).unwrap();
        let issue_event = CoordinationEventRecord {
            event_id: "issue-1",
            idempotency_key: "issue-key-1",
            aggregate_type: "campaign",
            aggregate_id: "campaign-1",
            expected_version: 0,
            event_type: "research_cycle_issued",
            payload_json: &cycle_payload,
            occurred_at_ms: 150,
        };
        ledger
            .issue_research_cycle_audited(
                &chief,
                &cycle,
                &root,
                "config/capability-map.md",
                150,
                &issue_event,
            )
            .await
            .unwrap();
        let target = IssuedStageBriefTarget {
            cycle_id: "cycle-1",
            campaign_id: "campaign-1",
            challenge_id: "challenge-1",
            role: Role::Solver,
        };
        assert!(
            load_and_render_issued_brief(&ledger_path, target.clone(), 160)
                .await
                .is_ok()
        );

        let revoke_payload = serde_json::json!({ "cycle_id": "cycle-1" }).to_string();
        let revoke_event = CoordinationEventRecord {
            event_id: "revoke-1",
            idempotency_key: "revoke-key-1",
            aggregate_type: "campaign",
            aggregate_id: "campaign-1",
            expected_version: 1,
            event_type: "research_cycle_revoked",
            payload_json: &revoke_payload,
            occurred_at_ms: 170,
        };
        ledger
            .revoke_research_cycle_audited(&chief, "cycle-1", 170, &revoke_event)
            .await
            .unwrap();
        assert!(
            load_and_render_issued_brief(&ledger_path, target, 180)
                .await
                .is_err()
        );
        ledger.close().await;
        fs::remove_dir_all(root).unwrap();
    }
}
