//! Simulation and execution of items (SPEC sections 3 and 9).
//!
//! Both go through [`check`], so a simulation lists exactly what the execution would touch.
//! Every path is checked again right before deletion: the disk may have changed since the scan.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use crate::guards;
use crate::process;
use crate::safety::{self, SafetyError, SafetyPolicy};
use crate::scan::{describe_blocked, Action, Disposal, Item};

/// Official commands may be slow (Docker, Flutter), but never hang forever.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Done,
    /// Some paths were cleaned, others skipped or failed.
    Partial,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub id: String,
    pub status: Status,
    /// Paths deleted or moved to the trash, or `1` for a command that succeeded.
    pub cleaned: usize,
    /// One line per path skipped or failed, and why.
    pub messages: Vec<String>,
}

/// What a simulation shows for one item.
#[derive(Debug, Clone, Serialize)]
pub struct Preview {
    pub id: String,
    /// Paths that would be deleted, or the command that would run.
    pub actions: Vec<String>,
    pub disposal: Option<Disposal>,
    /// Paths that would be skipped, and why.
    pub skipped: Vec<String>,
}

/// Resolves `path` if it may be deleted now, or says why it is skipped.
fn check(path: &Path, policy: &SafetyPolicy, project_guards: bool) -> Result<PathBuf, String> {
    let real = policy.check_deletable(path).map_err(|e| match e {
        SafetyError::Missing(_) => format!("{} : déjà supprimé.", path.display()),
        e => format!("{} : refusé par sécurité ({e}).", path.display()),
    })?;
    if project_guards {
        if !guards::git_available() {
            return Err(format!(
                "{} : git n'est pas disponible pour vérifier le projet.",
                path.display()
            ));
        }
        guards::check_artifact(path)
            .map_err(|blocked| format!("{} : {}", path.display(), describe_blocked(&blocked)))?;
    }
    Ok(real)
}

fn policy_for(home: &Path, extra_roots: &[PathBuf]) -> SafetyPolicy {
    extra_roots
        .iter()
        .fold(SafetyPolicy::new(home), |p, root| p.allow_root(root))
}

pub fn preview(item: &Item, home: &Path) -> Preview {
    let mut preview = Preview {
        id: item.id.clone(),
        actions: Vec::new(),
        disposal: item.action.disposal(),
        skipped: Vec::new(),
    };
    match &item.action {
        Action::Delete {
            paths,
            extra_roots,
            project_guards,
            ..
        } => {
            let policy = policy_for(home, extra_roots);
            for path in paths {
                match check(path, &policy, *project_guards) {
                    Ok(real) => preview.actions.push(real.display().to_string()),
                    Err(reason) => preview.skipped.push(reason),
                }
            }
        }
        Action::Command { .. } => preview.actions = item.action.describe(),
    }
    preview
}

pub fn execute(item: &Item, home: &Path) -> Report {
    if let Some(reason) = &item.blocked {
        return Report {
            id: item.id.clone(),
            status: Status::Skipped,
            cleaned: 0,
            messages: vec![reason.clone()],
        };
    }
    match &item.action {
        Action::Delete {
            paths,
            disposal,
            extra_roots,
            project_guards,
        } => {
            let mut cleaned = 0;
            let mut skipped = 0;
            let mut failed = 0;
            let mut messages = Vec::new();
            let policy = policy_for(home, extra_roots);
            for path in paths {
                let real = match check(path, &policy, *project_guards) {
                    Ok(real) => real,
                    Err(reason) => {
                        skipped += 1;
                        messages.push(reason);
                        continue;
                    }
                };
                let result = match disposal {
                    Disposal::Delete => safety::remove(&real).map_err(|e| describe_io(&e)),
                    Disposal::Trash => trash::delete(&real).map_err(|e| e.to_string()),
                };
                match result {
                    Ok(()) => cleaned += 1,
                    Err(reason) => {
                        failed += 1;
                        messages.push(format!("{} : {reason}", path.display()));
                    }
                }
            }
            let status = match (cleaned, skipped + failed) {
                (0, _) if failed > 0 => Status::Failed,
                (0, _) => Status::Skipped,
                (_, 0) => Status::Done,
                _ => Status::Partial,
            };
            Report {
                id: item.id.clone(),
                status,
                cleaned,
                messages,
            }
        }
        Action::Command { program, args } => {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            match process::run(program, &args, home, COMMAND_TIMEOUT) {
                Ok(output) if output.status.success() => Report {
                    id: item.id.clone(),
                    status: Status::Done,
                    cleaned: 1,
                    messages: Vec::new(),
                },
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Report {
                        id: item.id.clone(),
                        status: Status::Failed,
                        cleaned: 0,
                        messages: vec![describe_command_error(program, &stderr)],
                    }
                }
                Err(e) => Report {
                    id: item.id.clone(),
                    status: Status::Failed,
                    cleaned: 0,
                    messages: vec![describe_io(&e)],
                },
            }
        }
    }
}

fn describe_io(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => {
            "accès refusé (fichier protégé par le système ou utilisé par une application).".into()
        }
        std::io::ErrorKind::NotFound => "introuvable.".into(),
        std::io::ErrorKind::TimedOut => "la commande n'a pas terminé à temps.".into(),
        _ => error.to_string(),
    }
}

/// Turns known failures into plain language (real cases from the founding session).
fn describe_command_error(program: &str, stderr: &str) -> String {
    if stderr.contains("Cannot connect to the Docker daemon") || stderr.contains("docker daemon is not running") {
        return "Docker n'est pas lancé. Ouvrez Docker Desktop puis réessayez.".into();
    }
    if stderr.contains("Corepack") || stderr.contains("packageManager") {
        return format!("{program} a refusé de s'exécuter : un projet impose un autre gestionnaire de paquets.");
    }
    let last = stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("erreur inconnue");
    format!("{program} a échoué : {}", last.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Risk;
    use std::fs;

    fn delete_item(paths: Vec<PathBuf>, project_guards: bool) -> Item {
        Item {
            id: "test".into(),
            rule: "test".into(),
            title: "test".into(),
            detail: None,
            project: None,
            risk: Risk::None,
            size: 0,
            size_known: true,
            blocked: None,
            action: Action::delete(paths, Risk::None, Vec::new(), project_guards),
        }
    }

    #[test]
    fn simulation_matches_execution() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        let cache = home.join("Library/Caches/app");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("f"), "x").unwrap();
        let item = delete_item(
            vec![cache.clone(), home.join("Documents"), home.join("Library/Caches/gone")],
            false,
        );

        let preview = preview(&item, &home);
        assert_eq!(preview.actions, vec![cache.display().to_string()]);
        assert_eq!(preview.skipped.len(), 2);
        assert!(cache.exists(), "a simulation deletes nothing");

        let report = execute(&item, &home);
        assert_eq!(report.status, Status::Partial);
        assert_eq!(report.cleaned, 1);
        assert!(!cache.exists());
        assert!(home.join("Library").exists());
    }

    #[test]
    fn blocked_items_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        let mut item = delete_item(vec![home.join("x")], false);
        item.blocked = Some("raison".into());
        let report = execute(&item, &home);
        assert_eq!(report.status, Status::Skipped);
        assert_eq!(report.messages, vec!["raison"]);
    }

    #[test]
    fn explains_known_command_errors() {
        let message = describe_command_error(
            "docker",
            "Cannot connect to the Docker daemon at unix:///var/run/docker.sock.",
        );
        assert!(message.starts_with("Docker n'est pas lancé"));
        assert_eq!(
            describe_command_error("npm", "npm ERR! boom\n\n"),
            "npm a échoué : npm ERR! boom"
        );
    }
}
