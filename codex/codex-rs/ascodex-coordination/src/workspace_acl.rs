//! Role-specific workspace visibility and write boundaries.
//!
//! This module is deliberately independent of Codex's executor.  It produces a canonical,
//! fail-closed boundary that Core can translate into a managed filesystem permission profile.
//! `workspace_roots` alone are not an ACL: the built-in read-only profile grants root read access,
//! so callers must use the returned read roots when constructing the effective policy.

use crate::Role;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorkspaceAclError {
    #[error("challenge workspace must be an absolute directory")]
    InvalidChallengeRoot,
    #[error("clean-room role has no readable challenge inputs")]
    MissingCleanRoomInputs,
    #[error("read-only role has no readable evidence roots")]
    MissingEvidenceRoots,
    #[error("clean-room role requires at least one known parent workspace root")]
    MissingParentRoots,
    #[error("clean-room workspace overlaps an inherited parent workspace")]
    CleanRoomOverlapsParent,
}

/// Canonical role boundary.  The lists are explicit paths, never symbolic `:workspace_roots`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAcl {
    pub role: Role,
    pub challenge_root: PathBuf,
    pub cwd: PathBuf,
    pub readable_roots: Vec<PathBuf>,
    pub writable_roots: Vec<PathBuf>,
}

impl WorkspaceAcl {
    /// Derive the least-visible useful boundary from a challenge workspace.
    ///
    /// Missing optional evidence directories are ignored, but a role with no usable roots fails
    /// closed.  The returned paths are canonical, so a later executor translation cannot be
    /// tricked by `..` components or symlink aliases.
    pub fn for_role(challenge_root: &Path, role: Role) -> Result<Self, WorkspaceAclError> {
        if !challenge_root.is_absolute() {
            return Err(WorkspaceAclError::InvalidChallengeRoot);
        }
        let challenge_root = fs::canonicalize(challenge_root)
            .map_err(|_| WorkspaceAclError::InvalidChallengeRoot)?;
        if !challenge_root.is_dir() {
            return Err(WorkspaceAclError::InvalidChallengeRoot);
        }

        let mut readable_roots = Vec::new();
        let mut writable_roots = Vec::new();
        match role {
            Role::Solver => {
                readable_roots.push(challenge_root.clone());
                writable_roots.push(challenge_root.clone());
            }
            Role::RedTeam => {
                // Raw inputs only.  In particular, do not include outputs, trace, execution,
                // sub_agent, or any report directory that could contain the solver's answer.
                for relative in ["knowledge", "src"] {
                    push_existing(&challenge_root, relative, &mut readable_roots);
                }
                push_existing(
                    &challenge_root,
                    "characterization.json",
                    &mut readable_roots,
                );
                if readable_roots.is_empty() {
                    return Err(WorkspaceAclError::MissingCleanRoomInputs);
                }
            }
            Role::Monitor | Role::Intel | Role::JudgeAnalyst => {
                // Evidence readers see saved observations and manifests, not solver source or
                // mutable outputs.  The .agents skill directory is added by Core separately from
                // the signed StageBrief references.
                for relative in [
                    "outputs",
                    "execution/results",
                    "trace",
                    "knowledge",
                    "arm_manifest.json",
                    "characterization.json",
                ] {
                    push_existing(&challenge_root, relative, &mut readable_roots);
                }
                if readable_roots.is_empty() {
                    return Err(WorkspaceAclError::MissingEvidenceRoots);
                }
            }
            Role::Chief => {
                // Chief is the coordinator, not a worker workspace.  Keep this boundary useful
                // for reading campaign metadata while leaving all writes to the control plane.
                readable_roots.push(challenge_root.clone());
            }
        }

        Ok(Self {
            role,
            cwd: challenge_root.clone(),
            challenge_root,
            readable_roots,
            writable_roots,
        })
    }

    /// Returns true when `path` is inside one of the canonical ACL roots.
    pub fn can_read(&self, path: &Path) -> bool {
        is_within_any(path, &self.readable_roots)
    }

    /// Returns true when `path` is inside a role's writable root.
    pub fn can_write(&self, path: &Path) -> bool {
        !self.writable_roots.is_empty() && is_within_any(path, &self.writable_roots)
    }

    /// Proves that a clean-room workspace cannot resolve to the solver's inherited workspace.
    /// Canonicalizing both sides catches symlink aliases and rejects missing parent roots rather
    /// than silently treating an unverified parent as isolated.
    pub fn validate_clean_room_isolation(
        &self,
        parent_roots: &[PathBuf],
    ) -> Result<(), WorkspaceAclError> {
        if self.role != Role::RedTeam {
            return Ok(());
        }
        if parent_roots.is_empty() {
            return Err(WorkspaceAclError::MissingParentRoots);
        }
        for parent in parent_roots {
            let parent =
                fs::canonicalize(parent).map_err(|_| WorkspaceAclError::MissingParentRoots)?;
            if self.challenge_root.starts_with(&parent) || parent.starts_with(&self.challenge_root)
            {
                return Err(WorkspaceAclError::CleanRoomOverlapsParent);
            }
        }
        Ok(())
    }
}

fn push_existing(root: &Path, relative: &str, destination: &mut Vec<PathBuf>) {
    let candidate = root.join(relative);
    let Ok(canonical) = fs::canonicalize(candidate) else {
        return;
    };
    if (canonical.is_dir() || canonical.is_file()) && canonical.starts_with(root) {
        if !destination.iter().any(|existing| existing == &canonical) {
            destination.push(canonical);
        }
    }
}

fn is_within_any(path: &Path, roots: &[PathBuf]) -> bool {
    let Ok(canonical) = fs::canonicalize(path) else {
        return false;
    };
    roots.iter().any(|root| canonical.starts_with(root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);
    impl TempDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn tempdir() -> TempDir {
        let path = std::env::temp_dir().join(format!(
            "ascodex-workspace-acl-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    fn fixture() -> TempDir {
        let dir = tempdir();
        for relative in [
            "knowledge/problem.md",
            "src/input.json",
            "outputs/report.json",
            "execution/results/score.json",
            "trace/run.log",
            "sub_agent/solver.md",
        ] {
            let path = dir.path().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"fixture").unwrap();
        }
        fs::write(dir.path().join("characterization.json"), b"{}").unwrap();
        fs::write(dir.path().join("arm_manifest.json"), b"{}").unwrap();
        dir
    }

    #[test]
    fn solver_can_write_only_its_challenge_root() {
        let dir = fixture();
        let acl = WorkspaceAcl::for_role(dir.path(), Role::Solver).unwrap();
        assert!(acl.can_read(&dir.path().join("src/input.json")));
        assert!(acl.can_write(&dir.path().join("outputs/report.json")));
        assert!(!acl.can_read(&dir.path().parent().unwrap().join("other")));
    }

    #[test]
    fn red_team_excludes_solver_outputs_and_reports() {
        let dir = fixture();
        let acl = WorkspaceAcl::for_role(dir.path(), Role::RedTeam).unwrap();
        assert!(acl.can_read(&dir.path().join("src/input.json")));
        assert!(acl.can_read(&dir.path().join("knowledge/problem.md")));
        assert!(!acl.can_read(&dir.path().join("outputs/report.json")));
        assert!(!acl.can_read(&dir.path().join("trace/run.log")));
        assert!(!acl.can_read(&dir.path().join("sub_agent/solver.md")));
        assert!(!acl.can_write(&dir.path().join("src/input.json")));
    }

    #[test]
    fn evidence_roles_cannot_read_solver_source_or_write() {
        let dir = fixture();
        for role in [Role::Monitor, Role::Intel, Role::JudgeAnalyst] {
            let acl = WorkspaceAcl::for_role(dir.path(), role).unwrap();
            assert!(acl.can_read(&dir.path().join("trace/run.log")));
            assert!(!acl.can_read(&dir.path().join("src/input.json")));
            assert!(!acl.can_write(&dir.path().join("outputs/report.json")));
        }
    }

    #[test]
    fn missing_clean_room_inputs_fail_closed() {
        let dir = tempdir();
        fs::create_dir_all(dir.path().join("outputs")).unwrap();
        assert_eq!(
            WorkspaceAcl::for_role(dir.path(), Role::RedTeam).unwrap_err(),
            WorkspaceAclError::MissingCleanRoomInputs
        );
    }

    #[test]
    fn clean_room_must_not_overlap_parent_workspace() {
        let red_team = fixture();
        let separate_parent = tempdir();
        let acl = WorkspaceAcl::for_role(red_team.path(), Role::RedTeam).unwrap();
        assert!(
            acl.validate_clean_room_isolation(&[separate_parent.path().to_path_buf()])
                .is_ok()
        );
        assert_eq!(
            acl.validate_clean_room_isolation(&[red_team.path().to_path_buf()])
                .unwrap_err(),
            WorkspaceAclError::CleanRoomOverlapsParent
        );
        assert_eq!(
            acl.validate_clean_room_isolation(&[]).unwrap_err(),
            WorkspaceAclError::MissingParentRoots
        );
    }
}
