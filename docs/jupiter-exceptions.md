# Jupiter style exceptions

Chess Engine Compiler follows [Jupiter style standard 1.0](../../_standards/jupiter-style-standard.md) through the shared [Jupiter.UI](../../jupiter-ui/readme.md) library. The style check runs on every build with the default severity, Error, and the XAML has no `jupiter-allow` markers. These are the recorded departures.

| Rule | Where | Departure | Reason | Recorded |
| --- | --- | --- | --- | --- |
| 3.4 Content palettes | `Services/LogPresenter.cs` | Command lines and quoted literals in the build log use their own blue and green | Syntax highlighting of log content. Warnings and errors use the Jupiter status colors. | 24 October 2026 |
| 7 Suite workspace | `MainWindow.xaml` (Compile) | The Compile work area has two columns, options and build output, instead of one of the listed single-region patterns | Options and their live output are used together during a build; both stay visible. | 24 October 2026 |
