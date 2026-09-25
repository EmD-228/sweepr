//! Checks that run on every catalog file, at startup and in the tests.
//! A rule that fails here is a bug in the catalog, never something to work around at runtime.

use std::collections::HashSet;

use super::model::{Argv, CatalogFile, Ecosystem, Rule, Target};
use crate::paths::validate_pattern;

/// Characters that mean something to a shell. Commands never go through a shell,
/// but a catalog entry containing them is almost certainly a mistake.
const SHELL_METACHARACTERS: &[char] = &[';', '|', '&', '$', '`', '<', '>', '(', ')', '\n', '\r', '"', '\'', '*', '?'];

/// Artifact names that must never be deleted from a project, whatever an ecosystem says.
const PROTECTED_ARTIFACTS: &[&str] = &[".git", ".env", "src", "lib", "app", "ios", "android"];

pub fn validate_files(files: &[(&str, &CatalogFile)]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut ids = HashSet::new();
    for (name, file) in files {
        for rule in &file.rule {
            if !ids.insert(rule.id.clone()) {
                errors.push(format!("{name}: duplicate id `{}`", rule.id));
            }
            errors.extend(validate_rule(rule).into_iter().map(|e| format!("{name}: rule `{}`: {e}", rule.id)));
        }
        for eco in &file.ecosystem {
            if !ids.insert(eco.id.clone()) {
                errors.push(format!("{name}: duplicate id `{}`", eco.id));
            }
            errors.extend(validate_ecosystem(eco).into_iter().map(|e| format!("{name}: ecosystem `{}`: {e}", eco.id)));
        }
    }
    errors
}

fn validate_rule(rule: &Rule) -> Vec<String> {
    let mut errors = Vec::new();
    check_id(&rule.id, &mut errors);
    if rule.platforms.is_empty() {
        errors.push("`platforms` is empty".into());
    }
    for (field, value) in [
        ("title", &rule.title),
        ("summary", &rule.summary),
        ("loses", &rule.loses),
        ("regenerate", &rule.regenerate),
    ] {
        if value.trim().is_empty() {
            errors.push(format!("`{field}` is empty"));
        }
    }
    for program in &rule.requires {
        check_program(program, &mut errors);
    }
    match &rule.target {
        Target::Paths { paths, exclude } => {
            if paths.is_empty() {
                errors.push("`paths` is empty".into());
            }
            for pattern in paths.iter().chain(exclude) {
                if let Err(e) = validate_pattern(pattern) {
                    errors.push(e.to_string());
                }
            }
        }
        Target::Command { program, args, measure } => {
            check_argv(&Argv { program: program.clone(), args: args.clone() }, &mut errors);
            for pattern in measure {
                if let Err(e) = validate_pattern(pattern) {
                    errors.push(e.to_string());
                }
            }
        }
        Target::Provider { .. } => {}
    }
    errors
}

fn validate_ecosystem(eco: &Ecosystem) -> Vec<String> {
    let mut errors = Vec::new();
    check_id(&eco.id, &mut errors);
    if eco.name.trim().is_empty() || eco.regenerate.trim().is_empty() {
        errors.push("`name` and `regenerate` must not be empty".into());
    }
    if eco.detect.any_file.is_empty() {
        errors.push("`detect.any_file` is empty".into());
    }
    if eco.artifacts.is_empty() {
        errors.push("`artifacts` is empty".into());
    }
    for artifact in &eco.artifacts {
        let relative = artifact.strip_prefix("**/").unwrap_or(artifact);
        let components: Vec<&str> = relative.split('/').collect();
        let bad_component = components
            .iter()
            .any(|c| c.is_empty() || *c == "." || *c == ".." || c.contains(['*', '?', '[', '\\']));
        if relative.starts_with('/') || bad_component {
            errors.push(format!("artifact `{artifact}` must be a plain relative path"));
        }
        if relative.contains('/') && artifact.starts_with("**/") {
            errors.push(format!("artifact `{artifact}`: `**/` only works with a single name"));
        }
        if components.iter().any(|c| c.starts_with(".env") || *c == ".git") || PROTECTED_ARTIFACTS.contains(&relative) {
            errors.push(format!("artifact `{artifact}` is protected"));
        }
    }
    if let Some(argv) = &eco.official_command {
        check_argv(argv, &mut errors);
    }
    errors
}

fn check_id(id: &str, errors: &mut Vec<String>) {
    let valid = !id.is_empty()
        && id.split(['.', '-']).all(|part| {
            !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
    if !valid {
        errors.push(format!("id `{id}` must be lowercase words separated by `.` or `-`"));
    }
}

fn check_program(program: &str, errors: &mut Vec<String>) {
    let valid = !program.is_empty()
        && program.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !program.starts_with(['.', '-']);
    if !valid {
        errors.push(format!("program `{program}` must be a bare program name"));
    }
}

fn check_argv(argv: &Argv, errors: &mut Vec<String>) {
    check_program(&argv.program, errors);
    for arg in &argv.args {
        if arg.is_empty() || arg.contains(SHELL_METACHARACTERS) {
            errors.push(format!("argument `{arg}` of `{}` is empty or contains shell characters", argv.program));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml_text: &str) -> CatalogFile {
        toml::from_str(toml_text).expect("valid TOML")
    }

    const RULE: &str = r#"
        [[rule]]
        id = "macos.trash"
        profile = "general"
        category = "trash"
        platforms = ["macos"]
        title = "Corbeille"
        summary = "Fichiers déjà mis à la corbeille."
        risk = 0
        loses = "Les fichiers de la corbeille."
        regenerate = "Impossible."
        target = { kind = "paths", paths = ["~/.Trash/*"] }
    "#;

    #[test]
    fn accepts_a_valid_rule() {
        let file = parse(RULE);
        assert_eq!(validate_files(&[("t", &file)]), Vec::<String>::new());
    }

    #[test]
    fn rejects_duplicates_and_bad_values() {
        let file = parse(&format!("{RULE}{}", RULE.replace("~/.Trash/*", "~/*")));
        let errors = validate_files(&[("t", &file)]);
        assert!(errors.iter().any(|e| e.contains("duplicate id")));
        assert!(errors.iter().any(|e| e.contains("too broad")));
    }

    #[test]
    fn rejects_unknown_fields_and_out_of_range_risk() {
        assert!(toml::from_str::<CatalogFile>(&RULE.replace("risk = 0", "risk = 4")).is_err());
        assert!(toml::from_str::<CatalogFile>(&RULE.replace("risk = 0", "risk = 0\nsize = 3")).is_err());
    }

    #[test]
    fn rejects_shell_characters_in_commands() {
        let file = parse(
            &RULE.replace(
                r#"target = { kind = "paths", paths = ["~/.Trash/*"] }"#,
                r#"target = { kind = "command", program = "npm", args = ["cache", "clean; rm -rf ~"] }"#,
            ),
        );
        let errors = validate_files(&[("t", &file)]);
        assert!(errors.iter().any(|e| e.contains("shell characters")), "{errors:?}");

        let file = parse(&RULE.replace(
            r#"target = { kind = "paths", paths = ["~/.Trash/*"] }"#,
            r#"target = { kind = "command", program = "/bin/rm", args = ["-rf"] }"#,
        ));
        assert!(validate_files(&[("t", &file)]).iter().any(|e| e.contains("bare program name")));
    }

    #[test]
    fn rejects_protected_artifacts() {
        let file = parse(
            r#"
            [[ecosystem]]
            id = "bad"
            name = "Bad"
            detect = { any_file = ["package.json"] }
            artifacts = ["node_modules", ".git", "../outside", "**/.env.local", "src"]
            risk = 1
            regenerate = "x"
        "#,
        );
        let errors = validate_files(&[("t", &file)]);
        assert_eq!(errors.len(), 4, "{errors:?}");
    }
}
