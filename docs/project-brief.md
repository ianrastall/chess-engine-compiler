# Chess Engine Compiler

Requested by Ian on 24 October 2026 as a new Jupiter desktop application. This is a fresh repository at `D:/jupiter/chess-engine-compiler`. The older `D:/GitHub/StockfishCompiler` has a different name and scope and is left intact.

The first workflow is compiling Stockfish on Windows 11 x64. The user selects an official source tag, branch or commit, an MSYS2 UCRT64 installation, CPU architecture, parallel jobs, PGO and stripping. A build runs off the UI thread with phase updates, live output, cancellation and persistent logs. Success requires the resulting executable to answer `uci` and `isready`. Each job retains source, compiler identity, the resolved commit and build settings. No existing checkout or build is cleaned or overwritten.

The GUI exports a standalone CLI and PowerShell/Windows batch scripts carrying the selected settings. These call the same Rust command layer as the GUI. Build scripts are first-class supported interfaces, rather than a separate implementation of compilation rules.

## Stack and operating model

WinUI 3 / C# / CommunityToolkit.Mvvm shell, Rust domain core, narrow C ABI and generated Protocol Buffer messages. Windows 11 24H2 or later, x64. A per-user Inno Setup installer is mandatory and includes .NET, Windows App SDK and MSVC runtime files, a Jupiter Start Menu entry and registered uninstaller. A portable zip is supplemental. Stable application identity supports upgrades; user settings and engine outputs survive uninstall. One active build per GUI instance; independent instances and exported CLI runs use separate job folders. No background service or cloud build storage. GitHub authentication is optional for version metadata and binary downloads. Internet is required for upstream source and NNUE downloads. Compiler toolchains are detected or selected, never silently installed or updated.

Jupiter warm-dark controls, traditional menu bar, Windows accent tracking and the compact Settings > Appearance > Accent color chooser. Logs remain selectable and copyable; output and log folders are directly accessible. Shutdown asks before cancelling a running build, waits for the worker to stop, and preserves partial source/logs.

## Engine sequence

1. **Stockfish:** complete and verify the source-to-executable path, including regular and profile-guided builds, scripts and diagnostics.
2. **Lc0:** implement a separate adapter with dependency/backend detection and explicit CPU, CUDA and DirectML support as upstream permits. Network weights and runtime libraries must be identified independently from the compiled engine. Validate each backend on suitable hardware before advertising it.
3. **Other engines:** add candidates when their source licenses, documented build systems, Windows support and repeatable verification justify an adapter. Do not expose placeholder engines as working options.

## Original version 0.1 boundaries

Stockfish only. GCC via MSYS2 UCRT64 only; Clang and MSVC builds are not exposed. Upstream native CPU detection plus baseline x86-64, AVX2 and BMI2 presets; explicit presets must run on this CPU so PGO and UCI verification can execute safely. Default source is the verified `sf_19` tag, not a moving latest-release alias. Master or arbitrary reachable commits are opt-in. No tournament runner, chess GUI, engine editing, automatic tool installation or build queue in this release.

The result source archive contains tracked upstream source at the resolved commit. The full checked-out source and downloaded network stay in the job directory. Source scripts fetch their corresponding network. App publication distributes the compiler GUI/CLI, not compiled Stockfish engines or NNUE files. The build tool's MIT license does not change upstream licenses.

## Sources

- [Stockfish compilation guide](https://official-stockfish.github.io/docs/stockfish-wiki/Compiling-from-source.html)
- [Stockfish Makefile](https://github.com/official-stockfish/Stockfish/blob/master/src/Makefile)
- [Stockfish releases](https://github.com/official-stockfish/Stockfish/releases)
- [Lc0 build documentation and backend options](https://github.com/LeelaChessZero/lc0)
- [Berserk makefile](https://github.com/jhonnold/berserk/blob/main/src/makefile) and [networks](https://github.com/jhonnold/berserk-networks/releases)
- [Jupiter stack](../../_standards/stack-reference_winui3-rust-core.md) and [Jupiter style standard 1.0](../../_standards/jupiter-style-standard.md)

## Version 0.2: Lc0

Lc0 has a separate MSVC/Meson/Ninja adapter with explicit NVIDIA CUDA and CPU Eigen backends. Engine selection changes the configuration form; shared output/jobs/log controls stay in place. Stockfish selections remain independent. Lc0 defaults to v0.32.1, fetches its pinned common submodule, keeps weights separate and verifies the chosen backend with a real standalone network search. Result provenance includes network SHA-256, compiler/build-option records, dependency source archive and runtime list. See [the Lc0 workflow](lc0.md). Other backend variants are not advertised until implemented and verified.


## Version 0.3: hardware and GitHub workflows

Restore compatible CPU choices and automatic Stockfish recommendations. Present separate GitHub official-release and development source lists, plus manual refs. Add optional credential-manager token storage and GitHub CLI browser sign-in. A separate Downloads area retrieves upstream release assets or authorized workflow archives while builds continue independently. Verify published digests, preserve download provenance and never automatically run or extract downloaded binaries. Copy complete logs, colour the selectable GUI log and prefer PowerShell 7 for build process hosting when available.

## Version 0.4: Berserk

Requested by Ian on 24 October 2026 as the next useful engine. Berserk (GPL-3.0, C, makefile build) meets the "other engines" criteria: documented build, Windows support and repeatable `bench` verification. It reuses the MSYS2 UCRT64 GCC toolchain and Stockfish's form controls with Berserk's own CPU target names; parallel jobs are hidden because upstream compiles in one invocation. Its upstream network is downloaded, hash-checked against its file name and embedded; the static executable must pass UCI and a standalone search with a clean PATH. Clang is not exposed. See [the Berserk workflow](berserk.md).

## Additional engine: Obsidian

Obsidian support follows the Rust adapter and typed ABI model, with its own CPU names, embedded source-selected network, optional verified PGO and standalone UCI/benchmark verification. See [Obsidian](obsidian.md).

## Additional engine: Reckless

A separate Rust/Cargo adapter builds Reckless v0.9.0 with a static MSVC CRT and an embedded network. Initial support deliberately omits Syzygy and PGO; the form explains this and the core rejects unsupported PGO. See [Reckless](reckless.md).

## Additional engine: Igel

A GCC adapter builds Igel 3.8.0's standard embedded NNUE with Syzygy support. It requires AVX2/POPCNT, verifies the network SHA-256 and checks the standalone benchmark against the source commit's reference count when supplied. PGO/HCE/separate FRC-network variants are not exposed. See [Igel](igel.md).
