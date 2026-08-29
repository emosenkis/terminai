---
name: create-release
description: Use only when the user explicitly asks to create, cut, publish, or prepare a Terminai release. Do not use for requests that only ask to commit, push, or commit and push changes.
---

# Create Release

## Required Trigger

You MUST use this skill any time the user asks to create a release for this repository.

Do not use this skill when the user only asks to commit, push, or commit and push changes. Those requests are not release requests unless the user also explicitly asks for a release.

Before changing release files, confirm the release type is known:

- major
- minor
- patch

If the user did not specify major, minor, or patch, ask for that one missing detail and wait up to five minutes for a response. If no response arrives, use `patch`. Do not infer another release type from the change size.

## Workflow

1. Confirm the working tree state with `git status --short`.
2. Run the relevant checks before release metadata changes:
   - Format-check the Terminai package with
     `cargo fmt --manifest-path src/Cargo.toml -- --check`. Do not use
     `cargo fmt --all`; vendored code under `vendor/` is outside our formatting
     scope.
   - For the full project tests, prefer `cargo test` from `src/`.
3. Bump the version according to the specified release type:
   - `src/Cargo.toml`
   - `Cargo.lock`
   - any other tracked references to the package version that are intentionally versioned
4. Add a `CHANGELOG.md` entry for the new version with the current date and the *user-visible* changes.
   - If changelog validation rejects new Markdown syntax, decide deliberately
     whether that formatting is valuable enough to become supported syntax.
     Remove incidental formatting; add parser, renderer, and test support when
     the syntax materially improves the changelog and is likely to be reused.
     Do not bypass or weaken the validation test.
5. Re-run verification after the version and changelog updates.
6. Commit the release changes on `main` with a clear release commit message.
7. Push `main`.
8. Create a GitHub release for the new tag/version.
9. Wait for the GitHub release build workflow to complete successfully.
10. Verify the release artifacts exist and are usable.
11. Update the ignored `homebrew-tap/` checkout on a branch. Preserve the existing formula structure and update only the release-specific values.
12. Open a tap PR for the formula update. Do not stop for human review.
13. Wait for the tap `brew test-bot` workflow to complete.
    - Confirm every supported architecture completed successfully and produced bottle artifacts, not only passing checks.
    - Download or inspect artifacts and verify they contain `*.bottle.*.tar.gz` and `*.bottle.json`.
14. Publish bottles before merging the tap PR:
    - Immediately before dispatch, resolve the exact full PR head SHA with
      `gh pr view "$PR_NUMBER" --repo emosenkis/homebrew-tap --json headRefOid --jq .headRefOid`.
    - Pass that value unchanged as the workflow's `head_sha` input. Never
      abbreviate, guess, or manually transcribe the SHA.
    - Run the tap's GitHub `brew pr-pull` workflow for the PR.
    - Wait for it to publish all supported bottles and push the bottle commit successfully.
15. Merge the tap PR only after the bottle publish step has succeeded. If the publish step already merged or pushed the required commits, verify `main` includes them.
16. Run `brew update`, then verify local install uses the bottle:
    - `brew info emosenkis/tap/terminai` must show `(bottled)`.
    - `brew fetch --force --bottle-tag=x86_64_linux emosenkis/tap/terminai` must fetch a bottle.
    - `brew reinstall emosenkis/tap/terminai` must show `Pouring ...bottle...`, not `cargo install`.
17. Report the released version, main release URL, tap PR, bottle workflow run, tap release URL, and final tap commit.

## Guardrails

- Do not create a release from an unverified or failing tree unless the user explicitly accepts the risk after seeing the failure.
- Do not skip the Homebrew tap update when artifacts are available.
- Keep the tap as an ignored checkout in `./homebrew-tap`; do not convert it to a submodule.
- If GitHub build artifacts are not available yet, poll the workflow/release state rather than guessing hashes.
- Do not leave the Homebrew bottle workflow at "checks passed" only. Confirm artifacts, bottle publishing, and a local bottle pour.
- Do not wait for human intervention or review during the release/tap PR flow unless credentials or repository permissions are missing.
