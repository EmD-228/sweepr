//! Path patterns used by the catalog.
//!
//! A pattern starts with a token that names a known base folder, followed by
//! `/`-separated components that may contain glob wildcards:
//!
//! - `~/Library/Caches/*`
//! - `%LOCALAPPDATA%/Microsoft/Windows/Explorer/thumbcache_*`
//! - `/Applications/Install macOS*.app`
//!
//! The base folder is escaped before globbing, so a home folder containing
//! spaces or brackets can never change what a pattern matches.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PatternError {
    #[error("pattern `{0}` must start with a known token such as `~/` or `%TEMP%/`")]
    UnknownToken(String),
    #[error("pattern `{0}` uses `{1}`, which is not set on this system")]
    UnsetToken(String, String),
    #[error("pattern `{0}` must not contain `..`, `**`, `\\` or empty components")]
    Forbidden(String),
    #[error("pattern `{0}` is too broad: the first component after the token must start with a literal name")]
    TooBroad(String),
    #[error("pattern `{0}` is not a valid glob: {1}")]
    Glob(String, String),
}

/// A base folder that patterns may start from.
struct Token {
    name: &'static str,
    /// Environment variable holding the folder, `None` for the home folder or a fixed path.
    var: Option<&'static str>,
    /// Fixed absolute path, for tokens that are plain folders.
    fixed: Option<&'static str>,
    /// Whether `<token>/*` is acceptable. True only for folders that hold nothing but disposable data.
    wildcard_children: bool,
}

const TOKENS: &[Token] = &[
    Token { name: "~", var: None, fixed: None, wildcard_children: false },
    Token { name: "%TEMP%", var: Some("TEMP"), fixed: None, wildcard_children: true },
    Token { name: "%LOCALAPPDATA%", var: Some("LOCALAPPDATA"), fixed: None, wildcard_children: false },
    Token { name: "%APPDATA%", var: Some("APPDATA"), fixed: None, wildcard_children: false },
    Token { name: "%USERPROFILE%", var: Some("USERPROFILE"), fixed: None, wildcard_children: false },
    Token { name: "%PROGRAMDATA%", var: Some("PROGRAMDATA"), fixed: None, wildcard_children: false },
    Token { name: "%WINDIR%", var: Some("WINDIR"), fixed: None, wildcard_children: false },
    Token { name: "%SYSTEMDRIVE%", var: Some("SYSTEMDRIVE"), fixed: None, wildcard_children: false },
    Token { name: "/Applications", var: None, fixed: Some("/Applications"), wildcard_children: false },
];

/// The folders tokens resolve to. Built from the real system, or by hand in tests.
#[derive(Debug, Clone)]
pub struct PathEnv {
    home: PathBuf,
    vars: HashMap<String, PathBuf>,
}

impl PathEnv {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        PathEnv { home: home.into(), vars: HashMap::new() }
    }

    /// Reads the home folder and the token variables of the running system.
    pub fn from_system() -> Option<Self> {
        let mut env = PathEnv::new(dirs::home_dir()?);
        for token in TOKENS {
            if let Some(var) = token.var {
                if let Some(value) = std::env::var_os(var) {
                    env.vars.insert(var.to_string(), PathBuf::from(value));
                }
            }
        }
        if !env.vars.contains_key("TEMP") {
            env.vars.insert("TEMP".to_string(), std::env::temp_dir());
        }
        Some(env)
    }

    pub fn with_var(mut self, name: &str, value: impl Into<PathBuf>) -> Self {
        self.vars.insert(name.to_string(), value.into());
        self
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// Splits a pattern into its base folder and a glob pattern with that base escaped.
    pub fn expand(&self, pattern: &str) -> Result<Expanded, PatternError> {
        let (token, rest) = split_token(pattern)?;
        let base = match (token.var, token.fixed) {
            (Some(var), _) => self
                .vars
                .get(var)
                .cloned()
                .ok_or_else(|| PatternError::UnsetToken(pattern.to_string(), token.name.to_string()))?,
            (None, Some(fixed)) => PathBuf::from(fixed),
            (None, None) => self.home.clone(),
        };
        let mut glob = glob::Pattern::escape(&base.to_string_lossy());
        if !rest.is_empty() {
            glob.push('/');
            glob.push_str(rest);
        }
        glob::Pattern::new(&glob).map_err(|e| PatternError::Glob(pattern.to_string(), e.msg.to_string()))?;
        Ok(Expanded { base, glob })
    }

    /// Returns the existing paths matched by a pattern, sorted. Unreadable entries are skipped.
    pub fn resolve(&self, pattern: &str) -> Result<Vec<PathBuf>, PatternError> {
        let expanded = self.expand(pattern)?;
        let options = glob::MatchOptions {
            case_sensitive: !cfg!(windows),
            require_literal_separator: true,
            require_literal_leading_dot: false,
        };
        let entries = glob::glob_with(&expanded.glob, options)
            .map_err(|e| PatternError::Glob(pattern.to_string(), e.msg.to_string()))?;
        let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).collect();
        paths.sort();
        Ok(paths)
    }

    /// Existing paths matched by any of `patterns`. Patterns that cannot be expanded here
    /// (a Windows token on macOS, for example) match nothing.
    pub fn resolve_all(&self, patterns: &[String]) -> Vec<PathBuf> {
        patterns.iter().flat_map(|p| self.resolve(p).unwrap_or_default()).collect()
    }

    /// Compiles a pattern once, to test many paths against it (exclusions).
    pub fn compile(&self, pattern: &str) -> Result<glob::Pattern, PatternError> {
        let expanded = self.expand(pattern)?;
        glob::Pattern::new(&expanded.glob).map_err(|e| PatternError::Glob(pattern.to_string(), e.msg.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expanded {
    /// The folder the token stands for.
    pub base: PathBuf,
    /// Full glob pattern, base escaped.
    pub glob: String,
}

fn split_token(pattern: &str) -> Result<(&'static Token, &str), PatternError> {
    for token in TOKENS {
        if let Some(after) = pattern.strip_prefix(token.name) {
            if after.is_empty() {
                return Ok((token, ""));
            }
            if let Some(rest) = after.strip_prefix('/') {
                return Ok((token, rest));
            }
        }
    }
    Err(PatternError::UnknownToken(pattern.to_string()))
}

/// Checks that a catalog pattern is well-formed and cannot match a whole base folder.
/// Does not need the real system: only the token name is inspected.
pub fn validate_pattern(pattern: &str) -> Result<(), PatternError> {
    let (token, rest) = split_token(pattern)?;
    if rest.is_empty() {
        return Err(PatternError::TooBroad(pattern.to_string()));
    }
    let components: Vec<&str> = rest.split('/').collect();
    if pattern.contains('\\')
        || components.iter().any(|c| c.is_empty() || *c == "." || *c == ".." || c.contains("**"))
    {
        return Err(PatternError::Forbidden(pattern.to_string()));
    }
    let starts_with_wildcard = components[0].starts_with(['*', '?', '[']);
    if starts_with_wildcard && !token.wildcard_children {
        return Err(PatternError::TooBroad(pattern.to_string()));
    }
    glob::Pattern::new(rest).map_err(|e| PatternError::Glob(pattern.to_string(), e.msg.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn validates_patterns() {
        assert!(validate_pattern("~/Library/Caches/*").is_ok());
        assert!(validate_pattern("~/.Trash/*").is_ok());
        assert!(validate_pattern("%TEMP%/*").is_ok());
        assert!(validate_pattern("/Applications/Install macOS*.app").is_ok());

        assert!(matches!(validate_pattern("~"), Err(PatternError::TooBroad(_))));
        assert!(matches!(validate_pattern("~/*"), Err(PatternError::TooBroad(_))));
        assert!(matches!(validate_pattern("/Applications/*"), Err(PatternError::TooBroad(_))));
        assert!(matches!(validate_pattern("~/Library/../Documents"), Err(PatternError::Forbidden(_))));
        assert!(matches!(validate_pattern("~/Library/**/Caches"), Err(PatternError::Forbidden(_))));
        assert!(matches!(validate_pattern("~//x"), Err(PatternError::Forbidden(_))));
        assert!(matches!(validate_pattern("/etc/hosts"), Err(PatternError::UnknownToken(_))));
        assert!(matches!(validate_pattern("~foo/bar"), Err(PatternError::UnknownToken(_))));
    }

    #[test]
    fn resolves_under_home_with_spaces_and_brackets() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("Client [Work] Files");
        let caches = home.join("Library/Caches");
        fs::create_dir_all(caches.join("com.spotify.client")).unwrap();
        fs::create_dir_all(caches.join(".hidden")).unwrap();
        fs::write(caches.join("loose file.bin"), b"x").unwrap();

        let env = PathEnv::new(&home);
        let found = env.resolve("~/Library/Caches/*").unwrap();
        let names: Vec<_> = found.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec![".hidden", "com.spotify.client", "loose file.bin"]);
    }

    #[test]
    fn resolves_env_tokens() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.tmp"), b"x").unwrap();
        let env = PathEnv::new("/nowhere").with_var("TEMP", dir.path());
        assert_eq!(env.resolve("%TEMP%/*").unwrap(), vec![dir.path().join("a.tmp")]);
        assert!(matches!(env.resolve("%LOCALAPPDATA%/x"), Err(PatternError::UnsetToken(_, _))));
    }

    #[test]
    fn matches_exclusions() {
        let env = PathEnv::new("/Users/me");
        let exclusion = env.compile("~/.gradle/gradle.properties").unwrap();
        assert!(exclusion.matches_path(Path::new("/Users/me/.gradle/gradle.properties")));
        assert!(!exclusion.matches_path(Path::new("/Users/me/.gradle/caches")));
    }
}
