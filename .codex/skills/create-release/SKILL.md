---
name: create-release
description: Use whenever the user asks to create, cut, publish, or prepare a Terminai release. Requires the user to specify whether the release is major, minor, or patch before updating versions.
---

# Create Release

## Required Trigger

You MUST use this skill any time the user asks to create a release for this repository.

Before changing release files, confirm the release type is known:
- major
- minor
- patch

If the user did not specify major, minor, or patch, stop and ask for that one missing detail. Do not infer it from the change size.

## Workflow

1. Confirm the working tree state with `git status --short`.
2. Run the relevant tests before release metadata changes. For the full project, prefer `cargo test` from `src/`.
3. Bump the version according to the specified release type:
   - `src/Cargo.toml`
   - `Cargo.lock`
   - any other tracked references to the package version that are intentionally versioned
4. Add a `CHANGELOG.md` entry for the new version with the current date and the user-visible changes.
5. Re-run verification after the version and changelog updates.
6. Commit the release changes on `main` with a clear release commit message.
7. Push `main`.
8. Create a GitHub release for the new tag/version.
9. Wait for the GitHub release build workflow to complete successfully.
10. Download or inspect the built release artifacts and compute the file hashes required by the Homebrew formula.
11. Update the local ignored `homebrew-tap/` checkout with the new formula version and hashes.
12. Commit and push the Homebrew tap update.
13. Report the released version, commit, release URL, build status, and tap commit.

## Guardrails

- Do not create a release from an unverified or failing tree unless the user explicitly accepts the risk after seeing the failure.
- Do not skip the Homebrew tap update when artifacts are available.
- Keep the tap as an ignored checkout in `./homebrew-tap`; do not convert it to a submodule.
- If GitHub build artifacts are not available yet, poll the workflow/release state rather than guessing hashes.
