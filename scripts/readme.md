# Exported engine build scripts

`engine.json` and the engine-named copy (for example `stockfish.json`) contain the exact options selected in the GUI. Edit paths when moving this folder to another computer.

- `check-tools.cmd` prints detected tools and default settings.
- `preview-build.cmd` validates the configuration and prints the steps.
- `build-engine.cmd` and the engine-named entry point (for example `build-berserk.cmd`) compile the engine and verify its UCI handshake and standalone search.
- `build-engine.ps1 -Mode Build|Plan|Doctor` provides the same operations in PowerShell, with optional `-Config` and `-Compiler` paths.

Scripts use the bundled CLI and the same Rust core as the GUI. Run them in a terminal to keep output visible after completion. Stockfish and Berserk compilation requires MSYS2 UCRT64 with git, make and mingw-w64-ucrt-x86_64-gcc (Berserk also wget), internet access and enough disk space for the selected source and networks. Existing job folders are retained. PGO and symbol stripping are selected in the JSON file.

An exported script build runs independently from the GUI. Closing a terminal may interrupt the build; retain its log and rerun the configuration to create a new job. The GUI can cancel its own build process tree.
