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

1. For a patch release, run the checked-in release script with each user-visible
   changelog bullet as a separate argument:
   `scripts/release.sh patch "First change" "Second change"`.
   The script checks the tree, formats and tests, bumps package metadata,
   regenerates both schemas, updates the changelog, verifies again, commits,
   pushes `main`, tags and pushes the release, waits for the GitHub workflow,
   and verifies that release artifacts exist. Do not repeat those steps manually.
2. For another release type, extend the script first rather than manually
   duplicating its workflow.
3. Keep changelog arguments focused on *user-visible* changes.
   - If changelog validation rejects new Markdown syntax, decide deliberately
     whether that formatting is valuable enough to become supported syntax.
     Remove incidental formatting; add parser, renderer, and test support when
     the syntax materially improves the changelog and is likely to be reused.
     Do not bypass or weaken the validation test.
4. Update the ignored `homebrew-tap/` checkout on a branch. Preserve the existing formula structure and update only the release-specific values.
5. Open a tap PR for the formula update. Do not stop for human review.
6. Wait for the tap `brew test-bot` workflow to complete.
    - Confirm every supported architecture completed successfully and produced bottle artifacts, not only passing checks.
    - Download or inspect artifacts and verify they contain `*.bottle.*.tar.gz` and `*.bottle.json`.
7. Publish bottles before merging the tap PR:
    - Immediately before dispatch, resolve the exact full PR head SHA with
      `gh pr view "$PR_NUMBER" --repo emosenkis/homebrew-tap --json headRefOid --jq .headRefOid`.
    - Pass that value unchanged as the workflow's `head_sha` input. Never
      abbreviate, guess, or manually transcribe the SHA.
    - Run the tap's GitHub `brew pr-pull` workflow for the PR.
    - Wait for it to publish all supported bottles and push the bottle commit successfully.
8. Merge the tap PR only after the bottle publish step has succeeded. If the publish step already merged or pushed the required commits, verify `main` includes them.
9. Run `brew update`, then verify local install uses the bottle:
    - `brew info emosenkis/tap/terminai` must show `(bottled)`.
    - `brew fetch --force --bottle-tag=x86_64_linux emosenkis/tap/terminai` must fetch a bottle.
    - `brew reinstall emosenkis/tap/terminai` must show `Pouring ...bottle...`, not `cargo install`.
10. Report the released version, main release URL, tap PR, bottle workflow run, tap release URL, and final tap commit.

## Guardrails

- Do not create a release from an unverified or failing tree unless the user explicitly accepts the risk after seeing the failure.
- Do not skip the Homebrew tap update when artifacts are available.
- Keep the tap as an ignored checkout in `./homebrew-tap`; do not convert it to a submodule.
- If GitHub build artifacts are not available yet, poll the workflow/release state rather than guessing hashes.
- Do not leave the Homebrew bottle workflow at "checks passed" only. Confirm artifacts, bottle publishing, and a local bottle pour.
- Do not wait for human intervention or review during the release/tap PR flow unless credentials or repository permissions are missing.
