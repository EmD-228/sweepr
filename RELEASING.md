# Releasing

For the maintainer. Releases are built by GitHub Actions from a tag; nothing is built or uploaded by hand.

## Publishing a version

1. On a branch, set the new version in `package.json` and `src-tauri/Cargo.toml` (the app reads its version from `package.json`). Move the "Unreleased" entries of `CHANGELOG.md` under the new version with today's date. Open a pull request and merge it.
2. Tag the merge commit on `main` and push the tag:

   ```sh
   git switch main && git pull
   git tag -a v0.1.0 -m "Sweepr 0.1.0"
   git push origin v0.1.0
   ```

3. The `Build` workflow checks that the tag matches both versions, builds a universal macOS `.dmg`, computes `SHA256SUMS.txt` and creates a **draft** release.
4. Open the draft on the Releases page, check the notes against the changelog, install the `.dmg` on a Mac, then publish.

Versions follow Semantic Versioning. Before 1.0, a minor bump (0.2.0) may change behaviour; a patch (0.1.1) only fixes.

## Signing and notarization

Without secrets, releases have an ad hoc signature: macOS asks users to confirm the first launch, and Full Disk Access has to be granted again after every update, because macOS ties the permission to the signing identity. For real distribution (SPEC, section 10), add these repository secrets. They need an Apple Developer account.

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | `base64 -i certificate.p12` of the "Developer ID Application" certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password of the `.p12` |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID` | Apple account email used for notarization |
| `APPLE_PASSWORD` | An app-specific password for that account |
| `APPLE_TEAM_ID` | The team id |

Sign with the same identity from then on: changing it makes macOS ask every user for Full Disk Access again.

## Repository settings

These live in the GitHub settings, not in the repository:

- **Branch protection on `main`**: require a pull request before merging, require the `check` status check (branch up to date), dismiss stale approvals, block force pushes and deletion, apply to administrators too. Code owner reviews stay off while there is a single maintainer: GitHub does not let authors approve their own pull requests, so the maintainer could not merge. Only people with write access can merge anyway.
- **Merge button**: allow squash merging only, and use the pull request title as the commit message.
- **Security**: turn on private vulnerability reporting, Dependabot alerts and secret scanning.
- **Actions**: default workflow permissions read-only (the release job asks for write itself).
