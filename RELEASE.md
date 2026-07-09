# Release Process

Terminai uses GitHub Releases as the source for Homebrew binaries.

## First release

1. Make sure `Cargo.toml` contains the intended release version.
2. Update `homebrew-tap/Formula/terminai.rb` if the version, download URLs,
   or checksums change. The `homebrew-tap/` checkout is a separate ignored
   clone of `github.com/emosenkis/homebrew-tap`.
3. Commit the changes on `main`.
4. Create and push a release tag, for example:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

5. Let `.github/workflows/release.yml` build and publish the release artifacts.
6. Verify the GitHub Release contains:
   - Linux `x86_64` tarball
   - macOS `x86_64` tarball
   - macOS `arm64` tarball
   - Windows `x86_64` zip, if the workflow succeeded
   - matching `.sha256` files
7. Commit and push the Homebrew formula change from `homebrew-tap/`.

## Notes

- Homebrew installability depends on the GitHub Release existing before users run
  `brew install emosenkis/tap/terminai`.
- The release workflow is tag-driven; a tag and the crate version should match.
