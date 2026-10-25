# Jupiter structure and style audit — 24 October 2026, updated for style standard 1.0

References: workspace AGENTS.md, jupiter-plan.md, canonical WinUI/Rust stack, [Jupiter style standard 1.0](../../_standards/jupiter-style-standard.md) and UI package proposal. The app uses the shared [Jupiter.UI](../../jupiter-ui/readme.md) library for every color, text style, spacing value, control style and the accent control; its style check runs on every build with no violations. Departures are in [jupiter-exceptions.md](jupiter-exceptions.md).

| Requirement | Implementation |
| --- | --- |
| Direct-root project, separate repository | `D:/jupiter/chess-engine-compiler`; old StockfishCompiler untouched |
| WinUI 3/C#, supported .NET, MVVM | WinUI app, pinned .NET 10, CommunityToolkit.Mvvm observable presentation properties |
| Rust domain ownership and narrow C ABI | Independent domain and CLI crates, protobuf-generated messages, ABI version check, owned result handles with managed copies |
| Installer and Start Menu | Default build requires Inno Setup, per-user Jupiter installation and Start Menu entry, stable identity, uninstall and upgrade support; portable zip is supplemental |
| Separate binaries and writable state | Installed app folder is never used for state; preferences under LocalAppData/Jupiter and user-selected output directory |
| Native Fluent controls and Jupiter resources | Native MenuBar, NavigationView, fields, selectors, InfoBar, ProgressBar, dialogs and scrollable options, styled by Jupiter.UI; no local palette |
| Functional copy, suite workspace layout | Concrete build/source/output commands; Compile and Downloads are navigation work areas with Settings in the footer (standard section 7); no in-window app title, slogan or placeholder engine support |
| Engine-specific configuration | All six engine forms show their actual toolchain/backend controls and retain independent selections; shared output, jobs, log and build actions remain in the same places |
| GitHub and downloads | Native source selectors, separate Downloads work area and the Settings page GitHub section with its access dialog; Rust owns HTTPS/verification/cancellation, Windows Credential Manager owns token persistence; compilation stays independent |
| Windows accent and compact chooser | Shared Jupiter.UI accent service and Settings > Appearance > Accent color control; a custom color never chosen starts from the Windows accent |
| Traditional menu and native window behavior | File, Edit, View, Build, Help in the standard order, Settings at File > Settings (Ctrl+,); native caption/system menu; 1200 × 800 first launch, 800 × 560 minimum, persisted/clamped previous bounds |
| Visible progress, cancellation and results | Explicit phases and indeterminate progress, Running/Cancelling/Completed/Cancelled/Failed states, real process-tree cancellation and persistent/copyable logs |
| Keyboard and scaling | Native input controls, labels and accessible log name; fields scroll and primary build actions remain fixed; first-launch layout inspected at 200% DPI |
| Release and handoff | Versioned installer/zip/checksums, locked dependencies, Windows CI, HANDOFF.md and documented data locations/upgrade route |

This audit corrected portable-only delivery, missing previous-window restoration and missing handoff documentation. No stack exception is selected. A complete high-contrast/screen-reader audit, mixed-monitor exercise, clean-machine install and code-signing trust test remain platform verification work; code inspection and a development-machine install are not presented as those tests.
