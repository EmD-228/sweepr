# Sweepr

An honest disk cleaner. Sweepr shows exactly what fills your disk and explains every cleanup action before it runs: what it deletes, what you lose, how risky it is, and how to get it back.

It covers everyday clutter (application caches, old installers, logs, phone backups) and, when it finds developer tools, project build folders, SDK caches, simulators and Docker.

Sweepr is free and open source under the [MIT License](LICENSE).

> **Status: early development.** v0.1 targets macOS. Windows comes next, Linux after. See [PLAN.md](PLAN.md).

## Download

Installers are published on the [Releases page](../../releases). Each release lists a SHA-256 checksum for every file; check it before installing:

```sh
shasum -a 256 Sweepr_*.dmg
```

### First launch on macOS

1. Open the `.dmg` and drag Sweepr to Applications.
2. If macOS says the app cannot be checked, right-click Sweepr in Applications, choose **Open**, then confirm. This only happens with builds that are not notarized yet.
3. To see the Trash and the Mail and Messages attachments, Sweepr needs **Full Disk Access**. The app shows a button that opens the right settings page. Without it, Sweepr still works on everything else.

## How Sweepr stays safe

Deleting files is not something to be casual about. These rules are enforced by the code, and every change is reviewed against them:

- **Nothing is deleted without your confirmation.** Items that may hold your own work must be ticked one by one; irreplaceable data needs a second confirmation.
- **Every cleanup can be simulated first.** The simulation lists the exact paths or command, and deletes nothing.
- **Risk comes from fixed rules**, on a four-level scale: none, low, to check, irreplaceable.
- **Only reviewed actions run.** Cleanup rules live in data files in [`src-tauri/catalog/`](src-tauri/catalog/). Commands are fixed argument lists, never passed through a shell, and the interface can only refer to items by id, never send a path or a command.
- **Every path is checked again right before deletion**: it must be inside your home folder (or a folder the rule names), never a protected folder such as Documents, and a symbolic link can never lead a deletion elsewhere.
- **Project folders are checked with git**: a folder containing files tracked by git, a `.env` file or its own repository is never deleted.
- **Data that may not come back goes to the Trash**, not straight to deletion.
- **The space really recovered is measured** on the disk after cleaning, not just estimated.

## Build from source

Requirements: macOS, Rust (stable), Node 22+, pnpm 10.

```sh
pnpm install
pnpm tauri dev      # run the app
```

To build the app for your own Mac:

```sh
pnpm tauri build --bundles app
# -> src-tauri/target/release/bundle/macos/Sweepr.app
```

The bundle gets an ad hoc signature. macOS ties Full Disk Access to the signing identity, so with an ad hoc signature it asks again after each rebuild. To keep the permission, sign with a certificate from your keychain (`security find-identity -v -p codesigning`):

```sh
APPLE_SIGNING_IDENTITY="Apple Development: Your Name (TEAMID)" pnpm tauri build --bundles app
```

To try the engine without the interface, this prints what Sweepr finds on your disk. It is read-only:

```sh
cd src-tauri && cargo run --release --example scan
```

## Tests and checks

The same commands gate every pull request in CI (`.github/workflows/build.yml`, job `check`):

```sh
pnpm check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## Project layout

| Path | What it holds |
|---|---|
| `src-tauri/catalog/` | Cleanup rules, as TOML data. Most contributions start here. |
| `src-tauri/src/` | The Rust engine: scan, safety checks, git guards, execution, gain measurement. It does not depend on Tauri, except `commands.rs` and `tray.rs`. |
| `src/` | The interface (SvelteKit, Svelte 5, Tailwind). |
| `design/icon/` | Source SVGs of the app and menu bar icons. |
| `SPEC.md`, `PLAN.md` | Specification and plan (in French). |

## Contributing

Contributions are welcome, especially new cleanup rules. Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## Security

If you find a way to make Sweepr delete something it should not, report it privately: see [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE) © 2026 EmD Sama.
