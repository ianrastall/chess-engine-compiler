# Chess Engine Compiler

Requested by Ian on 24 October 2026 as a new Jupiter desktop application. This is a fresh repository at `D:/jupiter/chess-engine-compiler`. The older `D:/GitHub/StockfishCompiler` has a different name and scope and is left intact.

The first workflow is compiling Stockfish on Windows 11 x64. The user selects an official source tag, branch or commit, an MSYS2 UCRT64 installation, CPU architecture, parallel jobs, PGO and stripping. A build runs off the UI thread with phase updates, live output, cancellation and persistent logs. Success requires the resulting executable to answer `uci` and `isready`. Each job retains source, compiler identity, the resolved commit and build settings. No existing checkout or build is cleaned or overwritten.

The GUI exports a standalone CLI and PowerShell/Windows batch scripts carrying the selected settings. These call the same Rust command layer as the GUI. Build scripts are first-class supported interfaces, rather than a separate implementation of compilation rules.

## Stack and operating model

WinUI 3 / C# / CommunityToolkit.Mvvm shell, Rust domain core, narrow C ABI and generated Protocol Buffer messages. Windows 11 24H2 or later, x64. Self-contained .NET and Windows App SDK portable folder. One active build per GUI instance; independent instances and exported CLI runs use separate job folders. No background service, account or cloud storage. Internet is required for upstream source and NNUE downloads. Compiler toolchains are detected or selected, never silently installed or updated.

Jupiter warm-dark controls, traditional menu bar, Windows accent tracking and the compact Settings > Appearance > Accent color chooser. Logs remain selectable and copyable; output and log folders are directly accessible. Shutdown asks before cancelling a running build, waits for the worker to stop, and preserves partial source/logs.

## Engine sequence

1. **Stockfish:** complete and verify the source-to-executable path, including regular and profile-guided builds, scripts and diagnostics.
2. **Lc0:** implement a separate adapter with dependency/backend detection and explicit CPU, CUDA and DirectML support as upstream permits. Network weights and runtime libraries must be identified independently from the compiled engine. Validate each backend on suitable hardware before advertising it.
3. **Other engines:** add candidates when their source licenses, documented build systems, Windows support and repeatable verification justify an adapter. Do not expose placeholder engines as working options.

## Boundaries of version 0.1

Stockfish only. GCC via MSYS2 UCRT64 only; Clang and MSVC builds are not exposed. Upstream native CPU detection plus baseline x86-64, AVX2 and BMI2 presets; explicit presets must run on this CPU so PGO and UCI verification can execute safely. Default source is the verified `sf_19` tag, not a moving latest-release alias. Master or arbitrary reachable commits are opt-in. No tournament runner, chess GUI, engine editing, automatic tool installation or build queue in this release.

The result source archive contains tracked upstream source at the resolved commit. The full checked-out source and downloaded network stay in the job directory. Source scripts fetch their corresponding network. App publication distributes the compiler GUI/CLI, not compiled Stockfish engines or NNUE files. The build tool's MIT license does not change upstream licenses.

## Sources

- [Stockfish compilation guide](https://official-stockfish.github.io/docs/stockfish-wiki/Compiling-from-source.html)
- [Stockfish Makefile](https://github.com/official-stockfish/Stockfish/blob/master/src/Makefile)
- [Stockfish releases](https://github.com/official-stockfish/Stockfish/releases)
- [Lc0 build documentation and backend options](https://github.com/LeelaChessZero/lc0)
- [Jupiter stack](../../_standards/stack-reference_winui3-rust-core.md) and [interface conventions](../../_standards/jupiter-style-and-interface_draft.md)
