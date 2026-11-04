# Chess Engine Compiler

Jupiter desktop GUI for compiling Stockfish and Lc0 on Windows 11 x64.

Version 1.0 opens on **Release library** and checks for stable releases in the background. Select one or both engines and choose **Build new releases**. The first queue includes the newest two releases of each engine. Later checks add newly published releases and keep verified results. Choose **Latest release per engine** or **All releases** to change the scope. See the [release workflow and recipe rules](docs/release-library.md).

Use **Advanced build** for individual tags, branches or commits, manual CPU targets, Lc0 backend and weights choices, and script export. Jobs show live output and save the executable, exact source commit, settings, source archive and log.

## Install and run

Follows Jupiter style standard 1.0 through the shared Jupiter.UI library. A [pinned source snapshot](build/dependencies/readme.md) is included and restored by the build, so a fresh checkout needs no sibling repository. Departures are in [docs/jupiter-exceptions.md](docs/jupiter-exceptions.md).

Run the `ChessEngineCompiler-<version>-win-x64-setup.exe` release installer. It installs for your Windows account without an administrator prompt. Open **Start > Chess Engine Compiler**, or search Start for **Chess Engine Compiler**. .NET, Windows App SDK and MSVC runtimes are included. Uninstall through Windows Settings > Apps > Installed apps. Running a newer installer upgrades the same application; settings and engine-build outputs are preserved on upgrade and uninstall. Close the app before upgrading.

A portable zip is supplied as an additional option. Extract it and keep its files together. The installer is the standard Jupiter delivery route.

Open **Settings > Build tools** to detect, download and explicitly install the required languages and compiler tools. Simple installs use WinGet; Zig and Meson/Ninja use private app folders. Native installer choices remain visible. See [build-tool setup](docs/build-tools.md). Nothing is installed automatically when selecting an engine or starting a build.

For manual GCC setup, install [MSYS2](https://www.msys2.org/). In its UCRT64 terminal, update with `pacman -Syu` (restart/repeat when prompted), then install:

```sh
pacman -S --needed git make mingw-w64-ucrt-x86_64-gcc
```

Git for Windows on PATH is also supported. Select the MSYS2 root folder in the app, such as `C:\msys64`. Source and network downloads need internet access. The default source tag is Stockfish 19 (`sf_19`, checked 24 October 2026). Choose `master` for current development source or enter an explicit commit. `native` uses upstream CPU detection; choose `x86-64` for broader binary compatibility.

Each build creates a new job folder below the selected output folder. **Open result folder** shows the verified executable and its record; **Open saved log** opens the full tool output. PGO runs Stockfish's own profiling workload. Regular builds skip profiling. Neither mode deletes previous jobs.

**Follow output** keeps the newest log lines visible. Scrolling up, dragging the log scrollbar or selecting text pauses the view so incoming updates do not interrupt reading or copying. Re-enable **Follow output** to catch up; compilation continues while the view is paused. The full log remains available through **Open saved log**. Hover over build options for their meaning and practical tradeoffs, including PGO, architecture, jobs and symbol stripping.

## Stockfish and Lc0

Stockfish uses MSYS2 UCRT64 GCC. Managed builds use a compatible source-supported CPU target and profile-guided optimization. The engine's selected NNUE files are embedded.

Lc0 uses MSVC and Meson/Ninja. Choose NVIDIA CUDA or CPU Eigen in **Advanced build** and select **Save settings** to apply the choice to the library. The library can supply a verified default network. Each result includes the weights and any required CUDA runtime libraries and must pass a real standalone search. See [Lc0 tools, networks and build instructions](docs/lc0.md).

**Settings > GitHub > Manage GitHub access** supports anonymous access, a masked personal access token saved in Windows Credential Manager, or an existing GitHub CLI account. **Sign in through browser** uses GitHub CLI's device-code flow. Tokens never enter build JSON, exported scripts or logs. Authentication raises API limits and enables authorized workflow downloads. See [GitHub access and downloads](docs/github.md).

The separate **Downloads** work area lists release assets and authenticated workflow artifacts for each engine. Downloads can run alongside compilation. Each download gets a new folder and SHA-256/provenance record; published digests are verified when provided. Archives are not automatically extracted or executed. Expired artifacts are unavailable.

**Copy log** copies the full saved log or the current preview. Command strings, warnings and errors are highlighted in the selectable GUI log. Build tools prefer PowerShell 7 when installed, with direct execution as the fallback; interactive UCI input stays direct. **View > Watch log in PowerShell** opens a separate colour-coded live viewer. PowerShell is not required by the installer.

## CLI and scripts

```powershell
.\chess-engine-compiler.exe doctor
.\chess-engine-compiler.exe plan .\stockfish.json
.\chess-engine-compiler.exe build .\stockfish.json
```

Use **File > Export build scripts** to export the chosen settings, CLI, `build-engine.cmd`, an engine-specific batch entry point, `preview-build.cmd`, `check-tools.cmd` and `build-engine.ps1`. The [example configurations](examples) document editable settings. Script builds run independently of the GUI. The selected engine’s tools and network paths must also be available on any computer running its scripts. Earlier single-build configurations retain their CLI adapters.

## Develop

Pinned .NET 10 SDK, Rust 1.99.1, Windows SDK and MSVC x64 build tools are required. Dependencies are locked.

```powershell
pwsh -File build/build.ps1
```

This runs formatting, Clippy, domain/ABI tests, managed/native integration checks and actual WinUI log viewport and engine-form tests, then publishes the required installer, a supplemental portable zip and SHA-256 checksums under `dist/`. Inno Setup 6 is required; packaging fails if it is missing. Use `-InnoCompiler <path-to-ISCC.exe>` for a custom installation. See [the brief](docs/project-brief.md), [architecture](docs/architecture.md), [Jupiter audit](docs/jupiter-conformance.md), [handoff](HANDOFF.md) and [validation record](docs/validation.md).

The app icon is an original neutral rook/compiler mark, with its vector source, PNG preview and multi-resolution ICO under `src/ChessEngineCompiler.App/Assets`. To regenerate those committed assets, run `python build/generate-icon.py` with Pillow installed; Python/Pillow are development-only and are not needed to build or run the app.

The application is MIT licensed. Engines, networks and runtime libraries retain upstream licenses. Engine outputs and toolchains are not bundled with the app. This is a new repository; the former StockfishCompiler repository remains intact.
