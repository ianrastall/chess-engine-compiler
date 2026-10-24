# Jupiter structure and style audit — 24 October 2026

References: workspace AGENTS.md, jupiter-plan.md, canonical WinUI/Rust stack, Jupiter style draft 0.1 and UI package proposal. The shared Jupiter.UI package remains a proposal; this app uses local semantic resources and native controls as directed, without inventing a shared package.

| Requirement | Implementation |
| --- | --- |
| Direct-root project, separate repository | `D:/jupiter/chess-engine-compiler`; old StockfishCompiler untouched |
| WinUI 3/C#, supported .NET, MVVM | WinUI app, pinned .NET 10, CommunityToolkit.Mvvm observable presentation properties |
| Rust domain ownership and narrow C ABI | Independent domain and CLI crates, protobuf-generated messages, ABI version check, owned result handles with managed copies |
| Installer and Start Menu | Default build requires Inno Setup, per-user Jupiter installation and Start Menu entry, stable identity, uninstall and upgrade support; portable zip is supplemental |
| Separate binaries and writable state | Installed app folder is never used for state; preferences under LocalAppData/Jupiter and user-selected output directory |
| Native Fluent controls and warm dark resources | Native MenuBar, fields, selectors, InfoBar, ProgressBar, dialogs and scrollable options; Jupiter window/surface/text/separator brushes and contrast-theme resources |
| Functional copy, focused utility layout | Concrete build/source/output commands; no slogan, decorative introduction, compulsory sidebar or placeholder engine support |
| Engine-specific configuration | Stockfish and Lc0 forms show their actual toolchain/backend controls and retain independent selections; shared output, jobs, log and build actions remain in the same places |
| Windows accent and compact chooser | Live UISettings accent tracking, persisted custom override, Settings > Appearance with required labels and reset action |
| Traditional menu and native window behavior | File, Build, Settings, Help; native caption/system menu; DPI-aware geometry and persisted/clamped previous window bounds |
| Visible progress, cancellation and results | Explicit phases and indeterminate progress, Running/Cancelling/Completed/Cancelled/Failed states, real process-tree cancellation and persistent/copyable logs |
| Keyboard and scaling | Native input controls, labels and accessible log name; fields scroll and primary build actions remain fixed; first-launch layout inspected at 200% DPI |
| Release and handoff | Versioned installer/zip/checksums, locked dependencies, Windows CI, HANDOFF.md and documented data locations/upgrade route |

This audit corrected portable-only delivery, missing previous-window restoration and missing handoff documentation. No stack exception is selected. A complete high-contrast/screen-reader audit, mixed-monitor exercise, clean-machine install and code-signing trust test remain platform verification work; code inspection and a development-machine install are not presented as those tests.
