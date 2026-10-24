# Chess Engine Compiler

Read `readme.md` and `docs/project-brief.md` first. Follow `D:/jupiter/jupiter-plan.md` and the shared stack/interface references under `D:/jupiter/_standards`.

This is a new application/repository. Leave `D:/GitHub/StockfishCompiler` intact. Stockfish is implemented first; Lc0 is next. Do not expose unsupported engine/backend controls as working features.

Rust owns build validation, source acquisition, process execution, cancellation and verification. The WinUI shell uses the typed protobuf C ABI; scripts use the Rust CLI. Keep argument handling shared and avoid interpolating user values into shell commands. Preserve previous job folders. Success requires a verified executable and build provenance.

Run `pwsh -File build/build.ps1` for formatting, Clippy, Rust tests, managed/native integration and portable publication. When changing engine invocation, also perform an actual source build in an ignored test folder and check the resulting manifest/log. Record validation and its limits in `docs/validation.md`.

Use functional UI copy and the shared Windows accent chooser. Preserve unrelated work. Ian authorizes commits, pushes, pulls and release publication under the workspace instructions.
