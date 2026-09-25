# Changelog

All notable changes to Sweepr are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## Unreleased

### Added

- Disk scan on macOS: application caches, logs, old installers, Mail downloads, Messages attachments, Trash.
- Developer module, turned on when developer tools are found: project build and dependency folders (Node, React Native, Flutter, Rust, Tauri, Python) and tool caches (npm, pnpm, Yarn, CocoaPods, Gradle, Dart, Xcode DerivedData, Docker build cache).
- Projects sorted into active and inactive, with git checks that keep tracked files, `.env` files and nested repositories.
- Simulation of every cleanup, confirmation by risk level, and measurement of the space really recovered.
- Storage overview with charts, menu bar icon.
