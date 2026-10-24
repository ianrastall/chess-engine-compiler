# Chess Engine Compiler handoff

Repository: https://github.com/ianrastall/chess-engine-compiler. Principal checkout: `D:/jupiter/chess-engine-compiler`. Version: 0.2.0. The old StockfishCompiler checkout is unchanged. Git history and release tags identify the exact source commit; About reports the managed build identity.

## Build and verify

`pwsh -File build/build.ps1` runs formatting, strict Clippy, Rust tests, managed/native tests and actual WinUI log viewport and engine-form tests, then produces installer, portable zip and checksums. Requires the pinned .NET SDK and Rust toolchain, MSVC x64 tools and Inno Setup 6. Pass `-InnoCompiler` for a custom compiler location. `-SkipChecks` only skips checks; it still builds the installer. Dependencies are recorded in Cargo.lock, Directory.Packages.props and NuGet lockfiles. Lc0 invokes the user-selected Python/Meson toolchain from Rust; the GUI itself has no Python runtime or Python domain adapter; the optional icon regeneration script uses development-only Pillow.

Run `pwsh -File build/verify-install.ps1` to test install, repeated-install upgrade, uninstall/data preservation and final reinstall on the current account. It leaves the application installed. Refuses to operate on a pre-existing installation, so it cannot uninstall someone else's installation. The installer must already be built. For subsequent normal upgrades run the installer directly, with all app instances closed.

## Structure and ownership

`core/domain` owns builds and validation; `cli` calls it directly. `proto/compiler.proto` is the message contract, `core/ffi` ABI v1, and `src/ChessEngineCompiler.Interop` its LibraryImport/SafeHandle wrapper. The WinUI app owns presentation and native folder pickers. The shared [architecture record](docs/architecture.md) describes calls, cancellation and memory lifetimes. [The brief](docs/project-brief.md) owns product scope and the Stockfish → Lc0 sequence.

`Services/LogPresenter.cs` owns follow/pause behavior over a selectable TextBlock and external ScrollViewer. Pausing freezes the displayed snapshot and buffers the latest tail; resuming shows the newest snapshot after layout and scrolls to the bottom. Queued scroll callbacks recheck follow state. `tests/LogViewTests` exercises the real WinUI controls offscreen without keyboard/mouse automation or shared app preferences. The normal app publish uses isolated managed artifacts so a running development preview does not lock its build output.

## Installation and data

Installer: per-user, unpackaged WinUI, self-contained runtimes, JIT; no elevation. Binaries: `%LOCALAPPDATA%/Programs/Jupiter/ChessEngineCompiler`. Shortcut: current-user Programs/Jupiter/Chess Engine Compiler. Stable AppId/AppUserModelID and installer mutex: `Jupiter.ChessEngineCompiler` (mutex has Local prefix). Windows Installed apps registers an uninstaller. Installers update the same directory and identity; running instances must be closed first. GitHub release installers provide manual updates with published SHA-256 checksums. No automatic updater or signing certificate is configured.

UI/accent/window preferences and recent build selections: `%LOCALAPPDATA%/Jupiter/ChessEngineCompiler`. Engine sources, logs, results and archives: the selected output folder (default Documents/Chess engine builds). The installer never owns or deletes either location. Uninstall removes installed app files and shortcut, preserving user data. The Rust domain core is loaded only from beside the managed assemblies.

## Validation and limits

See [validation](docs/validation.md) and [Jupiter conformance](docs/jupiter-conformance.md) for the evidence and outstanding platform checks. Stockfish regular and PGO builds are verified. Lc0 supports MSVC/Meson/Ninja with CUDA or CPU Eigen; see docs/lc0.md for tools, network handling and backend limits. Stockfish uses MSYS2 UCRT64 GCC. Clean-machine runtime/signing trust, ARM, comprehensive accessibility and mixed-monitor DPI are not claimed as tested. Ian owns package/runtime servicing and republishing self-contained releases.

The real application also supports `--verify-ui <report-path>` for isolated offscreen engine-form verification and layout captures. It uses a separate state directory beside the report and never starts an engine build or changes normal user preferences. The default build runs this check on the published WinUI application.
