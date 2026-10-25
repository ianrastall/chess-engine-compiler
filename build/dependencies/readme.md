# Pinned Jupiter.UI build dependency

`jupiter-ui.zip` is an unmodified source archive of the shared `D:/jupiter/jupiter-ui` repository. `jupiter-ui.json` records its exact Git commit, library version and SHA-256. It includes the shared library, its build/style-check targets, SDK/package properties and NuGet lockfile. This is the canonical shared UI code, not app-defined Jupiter resources or a separate visual implementation.

`pwsh -File build/restore-jupiter-ui.ps1` verifies the archive and restores it below the ignored `target/dependencies/jupiter-ui` directory. Every build compares all cached source files with the archive, rejecting missing, changed or unexpected sources. Generated bin/obj files are allowed. The app and WinUI log harness import `$(JupiterUiRoot)build/Jupiter.UI.targets`; both local builds and CI use this same dependency. The sibling development repository is neither required nor modified.

To update, first commit reviewed changes in the shared repository, ensure its working tree is clean, and regenerate the source archive from that exact commit:

```powershell
git -C ../jupiter-ui archive --format=zip --output="$PWD/build/dependencies/jupiter-ui.zip" HEAD Directory.Build.props Directory.Packages.props global.json readme.md build src
```

Update `jupiter-ui.json` with that commit, version and archive SHA-256. Use a fresh `target/dependencies` cache for the new snapshot, then run the complete build and wait for GitHub Actions. Do not edit extracted library sources in this app. The shared repository remains the authoring location.
