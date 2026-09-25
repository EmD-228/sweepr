## What changes and why

<!-- Link the issue: "Closes #123". -->

## How it was tested

<!-- Tests added, and what you tried by hand. macOS version. Screenshots in light and dark mode for interface changes. -->

## Checklist

- [ ] One change in this pull request, nothing unrelated
- [ ] `pnpm check`, clippy and `cargo test` pass locally
- [ ] Follows the safety rules in CONTRIBUTING.md (confirmation, `check_deletable`, no command built from text, ids only from the interface)
- [ ] New cleanup rules: risk chosen honestly, `loses` and `regenerate` in plain language, and the PR explains why the folder is safe to delete
- [ ] User-visible change listed in CHANGELOG.md under "Unreleased"
