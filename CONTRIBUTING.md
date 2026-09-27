# Contributing to Sweepr

Thanks for helping. Sweepr deletes files on people's computers, so the bar for changes is safety first, then clarity. The process is short, and the rules below explain what a reviewer checks.

## How changes get in

1. **Open an issue first** for anything bigger than a small fix, so the approach is agreed before you spend time on it. To propose a new cleanup, use the "Cleanup rule" issue form.
2. **Fork the repository and branch from `main`.** Nobody commits to `main` directly, the maintainer included.
3. **Make your change and run the checks** (below). CI runs the same ones on every pull request.
4. **Open a pull request against `main`.** Fill in the template: what changes, why, how you tested it. Link the issue.
5. The maintainer reviews, may ask for changes, and **merges with a squash commit**. The PR title becomes the commit message.

## Pull request rules

- **One change per pull request.** No unrelated refactoring, formatting or dependency bumps mixed in.
- **The `check` job must pass**: type check, clippy with no warnings, tests.
- **New behaviour comes with tests.** Engine code is tested with `cargo test` on temporary folders and repositories, never on the real disk.
- **Title in the imperative, in English**, under 72 characters: `Add Homebrew cache rule`, `Fix size of hard-linked pnpm stores`.
- **User-visible changes get a line in [CHANGELOG.md](CHANGELOG.md)**, under "Unreleased".
- **Screenshots** for interface changes, in light and dark mode.
- Pull requests that touch deletion, safety checks or the catalog format need a review from the maintainer (see `.github/CODEOWNERS`) and are never merged in a hurry.

## Safety rules

A pull request is refused if it breaks any of these, whatever else it improves:

1. **Nothing is deleted without explicit confirmation.** Risk 2 items are ticked one by one; risk 3 needs the second confirmation.
2. **Every deletion goes through `SafetyPolicy::check_deletable`** (`src-tauri/src/safety.rs`), right before it happens.
3. **No command is built from text.** Commands are fixed `program` + `args` from the catalog, run with `std::process::Command`, never through a shell. A variable argument (a simulator id, a volume name) must come from the tool itself and be validated against a strict pattern.
4. **The interface never sends paths or commands** to the engine, only item ids from the last scan.
5. **Project folders go through the git guards** (`src-tauri/src/guards.rs`): tracked files, `.env` files and nested repositories block a deletion.
6. **Risk levels come from the catalog**, never from a heuristic in code.
7. **No network access, no telemetry, no AI.** Sweepr works offline and sends nothing anywhere.

## Adding a cleanup rule

Rules are data, in `src-tauri/catalog/*.toml`. They are embedded in the app and validated at startup and by `cargo test`: a malformed rule fails the tests.

```toml
[[rule]]
id = "dev.homebrew-cache"            # lowercase words separated by . or -
profile = "developer"                # general (everyone) or developer
category = "developer"               # caches, trash, temporary, personal, developer
platforms = ["macos"]
title = "Cache Homebrew"
summary = "Paquets téléchargés par Homebrew, gardés pour les réinstallations."
risk = 0                             # 0 none, 1 low, 2 to check, 3 irreplaceable
loses = "Rien. Homebrew retélécharge un paquet si besoin."
regenerate = "Automatique."
requires = ["brew"]                  # programs that must be installed
target = { kind = "command", program = "brew", args = ["cleanup", "--prune=all"], measure = ["~/Library/Caches/Homebrew"] }
```

- **Choose the risk honestly** (SPEC, section 6). When in doubt, pick the higher level.
- **`loses` and `regenerate` are read by people who are not developers** in simple mode. No jargon, no scare words: say what is lost and how it comes back.
- **Prefer the tool's official command** when one exists (`npm cache clean`, `docker builder prune`). Otherwise use `kind = "paths"`.
- **Path patterns start with a known token** (`~/`, `%LOCALAPPDATA%/`, `/Applications/`…) and use `/`. The first folder after the token must be a literal name: `~/*` is refused on purpose.
- **Explain in the pull request** how you checked that the folder is safe to delete: documentation, source code of the tool, or what happens when you delete it by hand.

## Conventions

- **Code, comments, commit messages and English documentation** are in English. The specification and the plan (`SPEC.md`, `PLAN.md`) are in French.
- **The interface is in French for now.** New text goes in plain language; translations will come with internationalization.
- **Svelte 5 with runes.** Styling with Tailwind; shared classes (`.btn`, `.card`…) live in `src/app.css`.
- **Icons come from `@lucide/svelte`.** No emoji in the interface.
- **No small label above a heading** (eyebrow or kicker). Context goes in the text below the heading.
- **The engine does not depend on Tauri.** Only `commands.rs` and `tray.rs` touch it, so the engine stays testable and a command-line version stays possible.

## Checks

```sh
pnpm install
pnpm check
cargo fmt --manifest-path src-tauri/Cargo.toml    # formats the Rust code; CI checks it
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## Reporting a security problem

Do not open a public issue. See [SECURITY.md](SECURITY.md).
