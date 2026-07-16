# Scoop installation

Each Windows GitHub release includes a checksum-pinned `terminai.json` Scoop
manifest next to `terminai-<version>-windows-x86_64.zip`.

After downloading that manifest, install it with:

```powershell
scoop install .\terminai.json
```

To publish Terminai through Scoop's standard `extras` bucket, copy the
release-generated manifest into the bucket and submit it there. The manifest
must be updated for every version because Scoop requires the exact archive
SHA-256.
