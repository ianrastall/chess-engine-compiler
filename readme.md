# Chess Engine Compiler

Jupiter desktop GUI for compiling chess engines on Windows 11 x64. Version 0.2 supports Stockfish and Lc0.

Select an official Stockfish tag, branch or commit, choose build options and compile. The GUI shows the current phase and live output, can cancel compilation, and saves the executable, exact source commit, compiler version, settings, source archive and full log. Export build scripts creates PowerShell and batch entry points with a standalone CLI and the selected configuration.

## Install and run

Run the `ChessEngineCompiler-<version>-win-x64-setup.exe` release installer. It installs for your Windows account without an administrator prompt. Open **Start > Jupiter > Chess Engine Compiler**, or search Start for **Chess Engine Compiler**. .NET, Windows App SDK and MSVC runtimes are included. Uninstall through Windows Settings > Apps > Installed apps. Running a newer installer upgrades the same application; settings and engine-build outputs are preserved on upgrade and uninstall. Close the app before upgrading.

A portable zip is supplied as an additional option. Extract it and keep its files together. The installer is the standard Jupiter delivery route.

Install [MSYS2](https://www.msys2.org/) separately. In its UCRT64 terminal, update with `pacman -Syu` (restart/repeat when prompted), then install:

```sh
pacman -S --needed git make mingw-w64-ucrt-x86_64-gcc
```

Git for Windows on PATH is also supported. Select the MSYS2 root folder in the app, such as `C:\msys64`. Source and network downloads need internet access. The default source tag is Stockfish 19 (`sf_19`, checked 24 October 2026). Choose `master` for current development source or enter an explicit commit. `native` uses upstream CPU detection; choose `x86-64` for broader binary compatibility.

Each build creates a new job folder below the selected output folder. **Open result folder** shows the verified executable and its record; **Open saved log** opens the full tool output. PGO runs Stockfish's own profiling workload. Regular builds skip profiling. Neither mode deletes previous jobs.

**Follow output** keeps the newest log lines visible. Scrolling up, dragging the log scrollbar or selecting text pauses the view so incoming updates do not interrupt reading or copying. Re-enable **Follow output** to catch up; compilation continues while the view is paused. The full log remains available through **Open saved log**. Hover over build options for their meaning and practical tradeoffs, including PGO, architecture, jobs and symbol stripping.

## Lc0

Select **Lc0** to show its backend, Python/Meson, Visual Studio, CUDA and network controls. Stockfish and Lc0 retain separate settings. Supported Lc0 backends are NVIDIA CUDA and CPU Eigen. Each result includes the selected network and any required CUDA runtime DLLs, and must pass a real standalone neural-network search. See [Lc0 tools, networks and build instructions](docs/lc0.md). The app does not install compiler tools.

## CLI and scripts

```powershell
.\chess-engine-compiler.exe doctor
.\chess-engine-compiler.exe plan .\stockfish.json
.\chess-engine-compiler.exe build .\stockfish.json
```

Use **File > Export build scripts** to export the chosen settings, CLI, `build-engine.cmd`, an engine-specific batch entry point, `preview-build.cmd`, `check-tools.cmd` and `build-engine.ps1`. The [example configuration](examples/stockfish.json) documents editable settings. Script builds run independently of the GUI. The selected engine’s tools and network paths must also be available on any computer running its scripts.

## Develop

Pinned .NET 10 SDK, Rust 1.99.1, Windows SDK and MSVC x64 build tools are required. Dependencies are locked.

```powershell
pwsh -File build/build.ps1
```

This runs formatting, Clippy, domain/ABI tests, managed/native integration checks and actual WinUI log viewport and engine-form tests, then publishes the required installer, a supplemental portable zip and SHA-256 checksums under `dist/`. Inno Setup 6 is required; packaging fails if it is missing. Use `-InnoCompiler <path-to-ISCC.exe>` for a custom installation. See [the brief](docs/project-brief.md), [architecture](docs/architecture.md), [Jupiter audit](docs/jupiter-conformance.md), [handoff](HANDOFF.md) and [validation record](docs/validation.md).

The app icon is an original neutral rook/compiler mark, with its vector source, PNG preview and multi-resolution ICO under `src/ChessEngineCompiler.App/Assets`. To regenerate those committed assets, run `python build/generate-icon.py` with Pillow installed; Python/Pillow are development-only and are not needed to build or run the app.

The application is MIT licensed. Engines, networks and runtime libraries retain upstream licenses. Engine outputs and toolchains are not bundled with the app. This is a new repository; the former StockfishCompiler repository remains intact.
