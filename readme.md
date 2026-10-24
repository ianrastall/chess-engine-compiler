# Chess Engine Compiler

Jupiter desktop GUI for compiling chess engines on Windows 11 x64. Version 0.1 supports Stockfish; Lc0 is next.

Select an official Stockfish tag, branch or commit, choose build options and compile. The GUI shows the current phase and live output, can cancel compilation, and saves the executable, exact source commit, compiler version, settings, source archive and full log. Export build scripts creates PowerShell and batch entry points with a standalone CLI and the selected configuration.

## Run

Extract the portable release and open `ChessEngineCompiler.exe`. Keep all files in the extracted folder. .NET and Windows App SDK runtimes are included.

Install [MSYS2](https://www.msys2.org/) separately. In its UCRT64 terminal, update with `pacman -Syu` (restart/repeat when prompted), then install:

```sh
pacman -S --needed git make mingw-w64-ucrt-x86_64-gcc
```

Git for Windows on PATH is also supported. Select the MSYS2 root folder in the app, such as `C:\msys64`. Source and network downloads need internet access. The default source tag is Stockfish 19 (`sf_19`, checked 24 October 2026). Choose `master` for current development source or enter an explicit commit. `native` uses upstream CPU detection; choose `x86-64` for broader binary compatibility.

Each build creates a new job folder below the selected output folder. **Open result folder** shows the verified executable and its record; **Open saved log** opens the full tool output. PGO runs Stockfish's own profiling workload. Regular builds skip profiling. Neither mode deletes previous jobs.

## CLI and scripts

```powershell
.\chess-engine-compiler.exe doctor
.\chess-engine-compiler.exe plan .\stockfish.json
.\chess-engine-compiler.exe build .\stockfish.json
```

Use **File > Export build scripts** to export the chosen settings, CLI, `build-stockfish.cmd`, `preview-build.cmd`, `check-tools.cmd` and `build-engine.ps1`. The [example configuration](examples/stockfish.json) documents editable settings. Script builds run independently of the GUI. MSYS2 must also be installed on any other computer running the scripts.

## Develop

Pinned .NET 10 SDK, Rust 1.99.1, Windows SDK and MSVC x64 build tools are required. Dependencies are locked.

```powershell
pwsh -File build/build.ps1
```

This runs formatting, Clippy, domain/ABI tests and managed/native integration checks, then publishes `dist/ChessEngineCompiler/` and a portable zip. See [the brief](docs/project-brief.md), [architecture](docs/architecture.md) and [validation record](docs/validation.md).

The application is MIT licensed. Stockfish and NNUE networks retain upstream licenses. Engine outputs and toolchains are not bundled with the app. This is a new repository; the former StockfishCompiler repository remains intact.
