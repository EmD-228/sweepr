//! Running external programs (git, xcrun, docker, npm...).
//!
//! A macOS app opened from the Finder receives a minimal `PATH` (`/usr/bin:/bin:...`), without
//! Homebrew, nvm, Flutter or Docker. The user's login shell `PATH` is read once and used for
//! every program Sweepr runs. Programs are always started directly, never through a shell,
//! with arguments passed one by one.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const MARKER: &str = "__SWEEPR_PATH__";

/// Folders where developer tools usually live, added when the login shell cannot be read.
const COMMON_DIRS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin", "~/.cargo/bin", "~/.volta/bin", "~/.local/bin"];

static SEARCH_PATH: OnceLock<OsString> = OnceLock::new();

/// The `PATH` used for every program Sweepr runs.
pub fn search_path() -> &'static OsString {
    SEARCH_PATH.get_or_init(|| {
        let current = std::env::var_os("PATH").unwrap_or_default();
        let mut dirs: Vec<_> = login_shell_path().map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
        dirs.extend(std::env::split_paths(&current));
        let home = dirs::home_dir();
        for dir in COMMON_DIRS {
            match (dir.strip_prefix("~/"), &home) {
                (Some(rest), Some(home)) => dirs.push(home.join(rest)),
                (None, _) => dirs.push(dir.into()),
                _ => {}
            }
        }
        let mut unique = Vec::new();
        for dir in dirs {
            if !unique.contains(&dir) {
                unique.push(dir);
            }
        }
        std::env::join_paths(unique).unwrap_or(current)
    })
}

#[cfg(unix)]
fn login_shell_path() -> Option<OsString> {
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/zsh".into());
    let mut child = Command::new(shell)
        .args(["-ilc", &format!("printf '{MARKER}%s{MARKER}' \"$PATH\"")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let status = wait_with_timeout(&mut child, Duration::from_secs(5))?;
    if !status.success() {
        return None;
    }
    let mut text = String::new();
    child.stdout.take()?.read_to_string(&mut text).ok()?;
    let start = text.find(MARKER)? + MARKER.len();
    let end = start + text[start..].find(MARKER)?;
    Some(OsString::from(&text[start..end]))
}

#[cfg(not(unix))]
fn login_shell_path() -> Option<OsString> {
    None
}

fn wait_with_timeout(child: &mut std::process::Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Whether `program` can be found in the search path.
pub fn has_program(program: &str) -> bool {
    find_program(program).is_some()
}

static PROGRAMS: OnceLock<Mutex<HashMap<String, Option<PathBuf>>>> = OnceLock::new();

/// Full path of `program` in the search path. Looked up once per program: git alone is
/// run hundreds of times per scan.
pub fn find_program(program: &str) -> Option<PathBuf> {
    let cache = PROGRAMS.get_or_init(Default::default);
    if let Some(found) = cache.lock().ok().and_then(|c| c.get(program).cloned()) {
        return found;
    }
    let extensions: &[&str] = if cfg!(windows) { &["exe", "cmd", "bat"] } else { &[""] };
    let found = std::env::split_paths(search_path()).find_map(|dir| {
        extensions.iter().find_map(|ext| {
            let candidate = if ext.is_empty() { dir.join(program) } else { dir.join(format!("{program}.{ext}")) };
            candidate.is_file().then_some(candidate)
        })
    });
    if let Ok(mut cache) = cache.lock() {
        cache.insert(program.to_string(), found.clone());
    }
    found
}

/// Builds a command for `program`, resolved in the search path, with a stable English output
/// and no interactive prompt.
pub fn command(program: &str, args: &[&str], cwd: &Path) -> io::Result<Command> {
    let resolved = find_program(program)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("`{program}` is not installed")))?;
    let mut command = Command::new(resolved);
    command
        .args(args)
        .current_dir(cwd)
        .env("PATH", search_path())
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null());
    Ok(command)
}

/// Runs `program` and returns its output, killing it after `timeout`.
pub fn run(program: &str, args: &[&str], cwd: &Path, timeout: Duration) -> io::Result<Output> {
    let mut child = command(program, args, cwd)?.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    // Read the pipes on threads so a verbose program cannot block on a full pipe.
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = stdout.as_mut() {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = stderr.as_mut() {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let status = wait_with_timeout(&mut child, timeout)
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, format!("`{program}` did not finish in time")))?;
    Ok(Output { status, stdout: out_reader.join().unwrap_or_default(), stderr: err_reader.join().unwrap_or_default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_a_program_without_shell() {
        let dir = tempfile::tempdir().unwrap();
        let (program, args): (&str, &[&str]) =
            if cfg!(windows) { ("cmd", &["/C", "echo", "a;b"]) } else { ("echo", &["a;b", "$HOME"]) };
        let output = run(program, args, dir.path(), Duration::from_secs(10)).unwrap();
        assert!(output.status.success());
        // Arguments are passed as-is: no shell splits `;` or expands `$HOME`.
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("a;b"));
        if cfg!(unix) {
            assert!(text.contains("$HOME"));
        }
    }

    #[test]
    fn reports_missing_programs() {
        let err = run("sweepr-definitely-not-a-program", &[], Path::new("."), Duration::from_secs(1)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[cfg(unix)]
    #[test]
    fn kills_programs_that_hang() {
        let err = run("sleep", &["5"], Path::new("."), Duration::from_millis(200)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }
}
