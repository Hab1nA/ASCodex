use crate::CoordinationError;
use crate::Role;
use crate::SCHEMA_VERSION;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const MAX_STAGE_BRIEF_BYTES: u32 = 1_229;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchStage {
    Intake,
    SolverExperiment,
    MonitorObservation,
    IntelObservation,
    PreSubmit,
    StuckJudge,
    StuckRedTeam,
    JudgeAnalysis,
    Closure,
    CloudCompute,
    Handover,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRef {
    pub name: String,
    pub source_path: String,
    pub sha256: String,
    pub selection_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageBrief {
    pub schema_version: String,
    pub brief_id: String,
    pub campaign_id: String,
    pub challenge_id: String,
    /// Canonicalizable absolute root for this role's challenge workspace. This is signed as part
    /// of the brief and is deliberately separate from the policy root used to verify skills.
    pub challenge_workspace_root: PathBuf,
    pub stage: ResearchStage,
    pub target_role: Role,
    pub generated_at_ms: i64,
    pub expires_at_ms: i64,
    pub max_bytes: u32,
    pub estimated_bytes: u32,
    pub skills: Vec<SkillRef>,
    pub selection_reason: String,
    pub capability_map_sha256: String,
    pub clean_room: bool,
}

impl StageBrief {
    pub fn validate(&self, now_ms: i64) -> Result<(), CoordinationError> {
        if self.schema_version != SCHEMA_VERSION
            || self.brief_id.trim().is_empty()
            || self.campaign_id.trim().is_empty()
            || self.challenge_id.trim().is_empty()
            || self.selection_reason.trim().is_empty()
            || !self.challenge_workspace_root.is_absolute()
        {
            return Err(invalid(
                "brief identifiers and selection reason are required",
            ));
        }
        if self.generated_at_ms < 0
            || self.expires_at_ms <= self.generated_at_ms
            || now_ms < self.generated_at_ms
            || now_ms >= self.expires_at_ms
        {
            return Err(invalid("brief is outside its validity window"));
        }
        if self.max_bytes == 0
            || self.max_bytes > MAX_STAGE_BRIEF_BYTES
            || self.estimated_bytes == 0
            || self.estimated_bytes > self.max_bytes
        {
            return Err(invalid("brief exceeds the bounded context budget"));
        }
        validate_digest(&self.capability_map_sha256, "capability map")?;

        let (allowed_roles, required_skills, clean_room) = route(self.stage);
        if !allowed_roles.contains(&self.target_role) || self.clean_room != clean_room {
            return Err(invalid(
                "brief target role or clean-room flag does not match its stage",
            ));
        }
        if self.skills.is_empty() {
            return Err(invalid("brief must select at least one skill"));
        }

        let mut names = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for skill in &self.skills {
            if skill.name.trim().is_empty()
                || skill.selection_reason.trim().is_empty()
                || !names.insert(skill.name.as_str())
                || !paths.insert(skill.source_path.as_str())
            {
                return Err(invalid(
                    "skill names, paths, and reasons must be non-empty and unique",
                ));
            }
            if skill.name.eq_ignore_ascii_case("worker-submit-chain")
                || skill
                    .source_path
                    .to_ascii_lowercase()
                    .contains("worker-submit-chain")
            {
                return Err(invalid(
                    "the retired worker-submit-chain skill is forbidden",
                ));
            }
            let path = Path::new(&skill.source_path);
            let normalized_path = skill.source_path.replace('\\', "/");
            let expected_path = format!(".agents/skills/{}/SKILL.md", skill.name);
            if path.is_absolute()
                || skill
                    .source_path
                    .split(['/', '\\'])
                    .any(|part| part == "..")
                || normalized_path != expected_path
            {
                return Err(invalid(
                    "skill source paths must be canonical workspace-relative paths",
                ));
            }
            validate_digest(&skill.sha256, "skill")?;
        }
        let actual = names.into_iter().collect::<BTreeSet<_>>();
        let expected = required_skills.iter().copied().collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(invalid(
                "brief skill set does not exactly match the stage route",
            ));
        }
        Ok(())
    }
}

fn route(stage: ResearchStage) -> (&'static [Role], &'static [&'static str], bool) {
    match stage {
        ResearchStage::Intake => (
            &[Role::Chief, Role::Intel, Role::JudgeAnalyst],
            &["playground-solve-optimal", "platform-scorecard-analyze"],
            false,
        ),
        ResearchStage::SolverExperiment => (
            &[Role::Solver],
            &["playground-solve-optimal", "platform-scorecard-analyze"],
            false,
        ),
        ResearchStage::MonitorObservation => (&[Role::Monitor], &["oracle-probe"], false),
        ResearchStage::IntelObservation => (&[Role::Intel], &["competition-coordinate"], false),
        ResearchStage::PreSubmit => (
            &[Role::Solver],
            &[
                "real-trace-capture",
                "trace-contamination-redline",
                "trace-maximize",
                "submit-attempt",
            ],
            false,
        ),
        ResearchStage::StuckJudge | ResearchStage::JudgeAnalysis => (
            &[Role::JudgeAnalyst],
            &[
                "platform-scorecard-analyze",
                "oracle-probe",
                "differential-scoring",
                "judge-field-audit",
            ],
            false,
        ),
        ResearchStage::StuckRedTeam => (
            &[Role::RedTeam],
            &["unstuck-switch-angle", "red-team-review"],
            true,
        ),
        ResearchStage::Closure => (
            &[Role::Chief, Role::Solver],
            &["closure-evidence-standard"],
            false,
        ),
        ResearchStage::CloudCompute => (&[Role::Solver], &["bohrium-bohr"], false),
        ResearchStage::Handover => (
            &[Role::Chief, Role::Solver, Role::JudgeAnalyst, Role::RedTeam],
            &["competition-coordinate"],
            false,
        ),
    }
}

pub(crate) fn validate_digest(value: &str, label: &str) -> Result<(), CoordinationError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid(&format!(
            "{label} sha256 must be a 64-character hexadecimal digest"
        )));
    }
    Ok(())
}

fn invalid(message: &str) -> CoordinationError {
    CoordinationError::InvalidDecision(format!("stage brief is invalid: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(name: &str) -> SkillRef {
        SkillRef {
            name: name.to_string(),
            source_path: format!(".agents/skills/{name}/SKILL.md"),
            sha256: "a".repeat(64),
            selection_reason: "required by the current stage".to_string(),
        }
    }

    fn brief(stage: ResearchStage, role: Role, names: &[&str]) -> StageBrief {
        StageBrief {
            schema_version: SCHEMA_VERSION.to_string(),
            brief_id: "brief-1".to_string(),
            campaign_id: "campaign-1".to_string(),
            challenge_id: "challenge-1".to_string(),
            challenge_workspace_root: std::env::temp_dir().join("ascodex-challenge-1"),
            stage,
            target_role: role,
            generated_at_ms: 100,
            expires_at_ms: 200,
            max_bytes: MAX_STAGE_BRIEF_BYTES,
            estimated_bytes: 900,
            skills: names.iter().map(|name| skill(name)).collect(),
            selection_reason: "stage route".to_string(),
            capability_map_sha256: "b".repeat(64),
            clean_room: stage == ResearchStage::StuckRedTeam,
        }
    }

    #[test]
    fn validates_exact_pre_submit_route() {
        let brief = brief(
            ResearchStage::PreSubmit,
            Role::Solver,
            &[
                "real-trace-capture",
                "trace-contamination-redline",
                "trace-maximize",
                "submit-attempt",
            ],
        );
        assert!(brief.validate(150).is_ok());
    }

    #[test]
    fn validates_solver_monitor_and_intel_routes_without_role_reuse() {
        let solver = brief(
            ResearchStage::SolverExperiment,
            Role::Solver,
            &["playground-solve-optimal", "platform-scorecard-analyze"],
        );
        let monitor = brief(
            ResearchStage::MonitorObservation,
            Role::Monitor,
            &["oracle-probe"],
        );
        let intel = brief(
            ResearchStage::IntelObservation,
            Role::Intel,
            &["competition-coordinate"],
        );
        assert!(solver.validate(150).is_ok());
        assert!(monitor.validate(150).is_ok());
        assert!(intel.validate(150).is_ok());

        let mut wrong_role = monitor;
        wrong_role.target_role = Role::Intel;
        assert!(wrong_role.validate(150).is_err());
    }

    #[test]
    fn rejects_retired_mixed_and_oversized_routes() {
        let mut mixed = brief(
            ResearchStage::StuckRedTeam,
            Role::RedTeam,
            &["unstuck-switch-angle", "worker-submit-chain"],
        );
        assert!(mixed.validate(150).is_err());
        mixed.skills = vec![skill("unstuck-switch-angle"), skill("red-team-review")];
        mixed.estimated_bytes = MAX_STAGE_BRIEF_BYTES + 1;
        assert!(mixed.validate(150).is_err());
    }

    #[test]
    fn rejects_wrong_role_expiry_path_and_digest() {
        let mut brief = brief(
            ResearchStage::CloudCompute,
            Role::JudgeAnalyst,
            &["bohrium-bohr"],
        );
        assert!(brief.validate(150).is_err());
        brief.target_role = Role::Solver;
        assert!(brief.validate(200).is_err());
        brief.expires_at_ms = 300;
        brief.skills[0].source_path = "../bohrium-bohr/SKILL.md".to_string();
        assert!(brief.validate(150).is_err());
        brief.skills[0].source_path = ".agents/skills/bohrium-bohr/SKILL.md".to_string();
        brief.skills[0].sha256 = "bad".to_string();
        assert!(brief.validate(150).is_err());

        brief.skills[0].sha256 = "a".repeat(64);
        brief.skills[0].source_path = "nested/.agents/skills/bohrium-bohr/SKILL.md".to_string();
        assert!(brief.validate(150).is_err());
    }
}
