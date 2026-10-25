using System.Diagnostics;
using System.Text.Json;
using Jupiter.ChessEngineCompiler.App.Services;
using Jupiter.ChessEngineCompiler.Interop;
using Jupiter.ChessEngineCompiler.Messages;
using Jupiter.UI;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Storage.Pickers;

namespace Jupiter.ChessEngineCompiler.App;

public sealed partial class MainWindow : Window
{
    private readonly CompilerCore _core = new();
    private readonly DispatcherQueueTimer _timer;
    private readonly AccentService _accent;
    private readonly ShellPreferences _preferences;
    private bool _active, _closing, _allowClose;
    private string _output = "", _logPath = "", _lastState = "Ready";
    private readonly LogPresenter _logPresenter;
    private string _engine = "stockfish";
    private bool _loadingOptions;
    private readonly Dictionary<string, BuildOptions> _engineOptions = new();
    private readonly bool _verification;
    private readonly GitHubCredentials _githubCredentials;
    private readonly GitHubSession _sourceSession = new();
    private Hardware _hardware = new();
    private readonly string DataFolder;
    private string ConfigPath => Path.Combine(DataFolder, "build-options.json");
    // Order matches the Engine combo box.
    private static readonly string[] Engines = ["stockfish", "lc0", "berserk", "obsidian", "reckless"];
    private static string EngineName(string engine) => engine switch { "lc0" => "Lc0", "berserk" => "Berserk", "obsidian" => "Obsidian", "reckless" => "Reckless", _ => "Stockfish" };
    private string OptionsPath(string engine) => engine == "stockfish" ? ConfigPath : Path.Combine(DataFolder, $"{engine}-options.json");
    private string RecommendedArchitecture => _engine == "reckless" ? _hardware.RecklessRecommended : _engine == "obsidian" ? _hardware.ObsidianRecommended : _engine == "berserk" ? _hardware.BerserkRecommended : _hardware.Recommended;
    public BuildViewModel Model { get; } = new();
    public MainWindow(string? dataFolder = null)
    {
        DataFolder = dataFolder ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Jupiter", "ChessEngineCompiler");
        _verification = dataFolder is not null;
        InitializeComponent();
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "app.ico"));
        _logPresenter = new LogPresenter(LogScroll, Log, FollowOutput, LogViewStatus);
        Title = "Chess Engine Compiler";
        Directory.CreateDirectory(DataFolder);
        _preferences = ShellPreferences.Load(Path.Combine(DataFolder, "ui.json"));
        _githubCredentials = new GitHubCredentials(DataFolder, _preferences);
        _accent = new AccentService(_preferences);
        _accent.Attach(Root);
        AccentSetting.Service = _accent;
        SettingsMenu.KeyboardAccelerators.Add(new Microsoft.UI.Xaml.Input.KeyboardAccelerator { Key = (Windows.System.VirtualKey)188, Modifiers = Windows.System.VirtualKeyModifiers.Control });
        UpdateGitHubAccess();
        _hardware = _core.Call(Request.Types.Operation.Hardware).Hardware;
        HardwareSummary.Text = _hardware.CpuName;
        DownloadPane.Initialize(_githubCredentials, _verification, WinRT.Interop.WindowNative.GetWindowHandle(this));
        var options = _core.Call(Request.Types.Operation.Defaults).Options;
        options.OutputRoot = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments), "Chess engine builds");
        try { if (File.Exists(ConfigPath)) options = ReadOptions(File.ReadAllText(ConfigPath)); }
        catch (Exception e) when (e is IOException or JsonException or InvalidOperationException) { ShowProblem("Saved options could not be read. Defaults have been restored."); }
        _engineOptions["stockfish"] = options.Clone();
        try
        {
            var lastPath = Path.Combine(DataFolder, "last-engine.txt");
            var last = File.Exists(lastPath) ? File.ReadAllText(lastPath) : "stockfish";
            if (last is "lc0" or "berserk" or "obsidian" or "reckless" && File.Exists(OptionsPath(last))) options = ReadOptions(File.ReadAllText(OptionsPath(last)));
        }
        catch (Exception e) when (e is IOException or JsonException or InvalidOperationException) { ShowProblem("Saved engine options could not be read. Stockfish settings have been restored."); }
        LoadOptions(options);
        Engine.SelectionChanged += Engine_Changed;
        if (!_verification) _ = LoadSourcesAsync(1);
        _timer = DispatcherQueue.CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(400);
        _timer.Tick += (_, _) => Refresh();
        _timer.Start();
        AppWindow.Closing += (window, e) => { if (!_allowClose) { e.Cancel = true; _ = CloseAsync(); } };
    }
    public void ConfigureWindow()
    {
        // Standard 6.1: a workspace opens at 1200 × 800 with a minimum of 800 × 560.
        var scale = WindowDpi.GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        var savedBounds = new Windows.Graphics.RectInt32(_preferences.WindowX, _preferences.WindowY, _preferences.WindowWidth, _preferences.WindowHeight);
        var monitor = _preferences.WindowX != int.MinValue && savedBounds.Width > 0 && savedBounds.Height > 0
            ? Microsoft.UI.Windowing.DisplayArea.GetFromRect(savedBounds, Microsoft.UI.Windowing.DisplayAreaFallback.Nearest)
            : Microsoft.UI.Windowing.DisplayArea.GetFromWindowId(AppWindow.Id, Microsoft.UI.Windowing.DisplayAreaFallback.Primary);
        var area = monitor.WorkArea;
        if (AppWindow.Presenter is Microsoft.UI.Windowing.OverlappedPresenter presenter)
        {
            presenter.PreferredMinimumWidth = Math.Min((int)(800 * scale), area.Width);
            presenter.PreferredMinimumHeight = Math.Min((int)(560 * scale), area.Height);
        }
        var width = Math.Min(Math.Max(_preferences.WindowWidth > 0 ? _preferences.WindowWidth : (int)(1200 * scale), (int)(800 * scale)), area.Width);
        var height = Math.Min(Math.Max(_preferences.WindowHeight > 0 ? _preferences.WindowHeight : (int)(800 * scale), (int)(560 * scale)), area.Height);
        var x = _preferences.WindowX == int.MinValue ? area.X + (area.Width - width) / 2 : Math.Clamp(_preferences.WindowX, area.X, area.X + area.Width - width);
        var y = _preferences.WindowY == int.MinValue ? area.Y + (area.Height - height) / 2 : Math.Clamp(_preferences.WindowY, area.Y, area.Y + area.Height - height);
        AppWindow.MoveAndResize(new Windows.Graphics.RectInt32(x, y, width, height));
    }
    private void SaveWindowBounds()
    {
        if (AppWindow.Presenter is Microsoft.UI.Windowing.OverlappedPresenter { State: Microsoft.UI.Windowing.OverlappedPresenterState.Restored })
        {
            _preferences.WindowX = AppWindow.Position.X; _preferences.WindowY = AppWindow.Position.Y;
            _preferences.WindowWidth = AppWindow.Size.Width; _preferences.WindowHeight = AppWindow.Size.Height;
            _preferences.Save();
        }
    }
    private void LoadOptions(BuildOptions options)
    {
        _loadingOptions = true;
        _engine = Engines.Contains(options.Engine) ? options.Engine : "stockfish";
        Engine.SelectedIndex = Array.IndexOf(Engines, _engine);
        ShowArchitectures();
        Model.SourceRef = options.SourceRef; Model.MsysRoot = options.MsysRoot; Model.OutputRoot = options.OutputRoot;
        Jobs.Value = options.Jobs; Pgo.IsChecked = options.Pgo; Strip.IsChecked = options.Strip;
        AutoArchitecture.IsChecked = options.AutoArchitecture;
        foreach (ComboBoxItem item in Architecture.Items) if ((string)item.Content == options.Architecture) Architecture.SelectedItem = item;
        PythonPath.Text = options.PythonPath; VisualStudio.Text = options.VisualStudio;
        CudaRoot.Text = options.CudaRoot; WeightsPath.Text = options.WeightsPath;
        Backend.SelectedIndex = options.Backend == "eigen" ? 1 : 0;
        // Stockfish and Berserk share the MSYS2 GCC controls; Berserk compiles in one invocation, so parallel jobs do not apply.
        StockfishOptions.Visibility = _engine != "lc0" ? Visibility.Visible : Visibility.Collapsed;
        MsysOptions.Visibility = _engine is "lc0" or "reckless" ? Visibility.Collapsed : Visibility.Visible;
        RustRequirements.Visibility = _engine == "reckless" ? Visibility.Visible : Visibility.Collapsed;
        Pgo.Visibility = _engine == "reckless" ? Visibility.Collapsed : Visibility.Visible;
        Lc0Options.Visibility = _engine == "lc0" ? Visibility.Visible : Visibility.Collapsed;
        Jobs.Visibility = _engine == "berserk" ? Visibility.Collapsed : Visibility.Visible;
        CompileButton.Content = StartMenu.Text = $"Compile {EngineName(_engine)}";
        SourceHint.Text = _engine switch
        {
            "lc0" => "Default: Lc0 v0.32.1. Network weights are selected separately below.",
            "reckless" => "Default: Reckless v0.9.0. Rust CPU build with an embedded network; Syzygy and PGO are not included.",
            "obsidian" => "Default: Obsidian v16.0. The source-selected network is downloaded and embedded; its SHA-256 is recorded.",
            "berserk" => "Default: Berserk 14. The network named by the source is downloaded, verified and embedded in the executable.",
            _ => "Default: Stockfish 19 (sf_19). Use master for development source, or enter a tag or commit.",
        };
        ToolTipService.SetToolTip(SourceRef, _engine switch
        {
            "lc0" => "Select an official LeelaChessZero/lc0 release tag, branch or exact commit. The default is v0.32.1. Its pinned lczero-common submodule is fetched with it.",
            "reckless" => "Select a codedeliveryservice/Reckless release tag, branch or exact commit. Default: v0.9.0.",
            "obsidian" => "Select a gab8192/Obsidian release tag, branch or exact commit. The default is v16.0.",
            "berserk" => "Select an official jhonnold/berserk release tag, branch or exact commit. The default is 14; main follows development.",
            _ => "Select an official Stockfish release tag, branch or exact commit. The default is sf_19; master follows development.",
        });
        _loadingOptions = false;
        AutoArchitecture_Changed(this, null!);
        Backend_Changed(this, null!);
    }
    private void ShowArchitectures()
    {
        Architecture.Items.Clear();
        foreach (var choice in _engine == "reckless" ? _hardware.RecklessChoices : _engine == "obsidian" ? _hardware.ObsidianChoices : _engine == "berserk" ? _hardware.BerserkChoices : _hardware.Choices) Architecture.Items.Add(new ComboBoxItem { Content = choice });
        Architecture.SelectedIndex = 0;
        ArchitectureExplanation.Text = _engine == "reckless" ? $"Recommended Reckless target: {_hardware.RecklessRecommended}. AVX2 uses vector evaluation; x86-64 favors compatibility." : _engine == "obsidian" ? $"Recommended Obsidian target: {_hardware.ObsidianRecommended}. Only compatible targets are offered; PEXT is avoided on older AMD CPUs." : _engine == "berserk" ? _hardware.BerserkExplanation : _hardware.Explanation;
    }
    private void Engine_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (_loadingOptions || _active) return;
        try
        {
            var previous = Options();
            _engineOptions[_engine] = previous;
            var engine = Engines[Math.Clamp(Engine.SelectedIndex, 0, Engines.Length - 1)];
            if (!_engineOptions.TryGetValue(engine, out var next))
            {
                next = _core.Call(Request.Types.Operation.Defaults, new BuildOptions { Engine = engine }).Options;
                next.OutputRoot = previous.OutputRoot; next.Jobs = previous.Jobs;
                var path = OptionsPath(engine);
                if (File.Exists(path)) next = ReadOptions(File.ReadAllText(path));
            }
            LoadOptions(next);
            _loadingSources = true;
            OfficialVersions.ItemsSource = DevelopmentVersions.ItemsSource = null;
            SourceChannel.SelectedIndex = 2;
            _loadingSources = false;
            SourceChannel_Changed(this, null!);
            if (!_verification) _ = LoadSourcesAsync(1);
            Notice.IsOpen = false;
        }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private void Backend_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (CudaRoot is not null && Backend is not null) CudaRoot.Visibility = Backend.SelectedIndex == 0 ? Visibility.Visible : Visibility.Collapsed;
    }
    private BuildOptions Options() => new()
    {
        Engine = _engine,
        SourceRef = Model.SourceRef.Trim(),
        MsysRoot = Model.MsysRoot.Trim(),
        OutputRoot = Model.OutputRoot.Trim(),
        Architecture = (string)((ComboBoxItem)Architecture.SelectedItem).Content,
        Jobs = double.IsFinite(Jobs.Value) ? (uint)Math.Clamp(Jobs.Value, 0, 256) : 0,
        Pgo = Pgo.IsChecked == true,
        Strip = Strip.IsChecked == true,
        Backend = Backend.SelectedIndex == 1 ? "eigen" : "cuda-auto",
        PythonPath = PythonPath.Text.Trim(), VisualStudio = VisualStudio.Text.Trim(),
        CudaRoot = CudaRoot.Text.Trim(), WeightsPath = WeightsPath.Text.Trim(),
        AutoArchitecture = AutoArchitecture.IsChecked == true,
    };
    private static BuildOptions ReadOptions(string json) => Google.Protobuf.JsonParser.Default.Parse<BuildOptions>(json);
    private void ShowProblem(string message) { Notice.Severity = InfoBarSeverity.Error; Notice.Title = "Build needs attention"; Notice.Message = message; Notice.IsOpen = true; }
    private void Preview_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            var preview = "Build preview\n\n" + string.Join("\n\n", _core.Call(Request.Types.Operation.Plan, Options()).Steps);
            _logPresenter.ShowPreview(preview);
            Notice.IsOpen = false;
        }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private void Start_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            var options = Options();
            _core.Call(Request.Types.Operation.Start, options);
            _logPresenter.BeginBuild();
            Notice.IsOpen = false;
            Refresh();
            try
            {
                File.WriteAllText(OptionsPath(_engine), ScriptExporter.SerializeOptions(options));
                File.WriteAllText(Path.Combine(DataFolder, "last-engine.txt"), _engine);
            }
            catch (IOException error) { ShowProblem($"Build started, but its options could not be saved: {error.Message}"); }
        }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private void Cancel_Click(object sender, RoutedEventArgs e)
    {
        try { _core.Call(Request.Types.Operation.Cancel); Refresh(); }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private void Refresh()
    {
        try
        {
            var s = _core.Call(Request.Types.Operation.Status).Snapshot;
            _active = s.State is "Running" or "Cancelling";
            Model.Status = $"{s.State} — {s.Phase}" + (s.Message.Length > 0 ? $" · {s.Message}" : "");
            Configuration.IsEnabled = CompileButton.IsEnabled = StartMenu.IsEnabled = PreviewButton.IsEnabled = PreviewMenu.IsEnabled = !_active;
            CancelButton.IsEnabled = CancelMenu.IsEnabled = s.State == "Running";
            Progress.IsIndeterminate = _active; Progress.Visibility = _active ? Visibility.Visible : Visibility.Collapsed;
            if (_active || _lastState is "Running" or "Cancelling") _logPresenter.Update(s.LogTail);
            _output = s.Output; _logPath = s.LogPath;
            ResultButton.IsEnabled = File.Exists(_output); LogButton.IsEnabled = File.Exists(_logPath);
            PowerShellLogMenu.IsEnabled = File.Exists(_logPath);
            if (_lastState != s.State && s.State is "Completed" or "Failed" or "Cancelled")
            {
                Notice.Severity = s.State == "Completed" ? InfoBarSeverity.Success : s.State == "Failed" ? InfoBarSeverity.Error : InfoBarSeverity.Informational;
                Notice.Title = s.State; Notice.Message = s.Message; Notice.IsOpen = true;
            }
            _lastState = s.State;
        }
        catch (Exception error) { _timer.Stop(); ShowProblem(error.Message); }
    }
    private async Task<string?> PickFolderAsync()
    {
        var picker = new FolderPicker(); picker.FileTypeFilter.Add("*");
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(this));
        return (await picker.PickSingleFolderAsync())?.Path;
    }
    private async void BrowseMsys_Click(object sender, RoutedEventArgs e) { try { if (await PickFolderAsync() is { } path) Model.MsysRoot = path; } catch (Exception error) { ShowProblem(error.Message); } }
    private async void BrowseOutput_Click(object sender, RoutedEventArgs e) { try { if (await PickFolderAsync() is { } path) Model.OutputRoot = path; } catch (Exception error) { ShowProblem(error.Message); } }
    private async void BrowseWeights_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            var picker = new FileOpenPicker(); picker.FileTypeFilter.Add(".gz"); picker.FileTypeFilter.Add(".pb");
            WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(this));
            if (await picker.PickSingleFileAsync() is { } file) WeightsPath.Text = file.Path;
        }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private void Detect_Click(object sender, RoutedEventArgs e)
    {
        if (_active) return;
        try
        {
            var detected = _core.Call(Request.Types.Operation.Defaults, new BuildOptions { Engine = _engine }).Options;
            if (_engine == "lc0")
            {
                PythonPath.Text = detected.PythonPath; VisualStudio.Text = detected.VisualStudio; CudaRoot.Text = detected.CudaRoot;
                if (PythonPath.Text.Length == 0 || VisualStudio.Text.Length == 0) ShowProblem("Lc0 build tools were not found. See Help > Build requirements and enter their paths.");
            }
            else { Model.MsysRoot = detected.MsysRoot; if (Model.MsysRoot.Length == 0) ShowProblem("MSYS2 UCRT64 was not found. Browse to its root folder or see Help > Build requirements."); }
        }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private async void Export_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            var options = Options(); _core.Call(Request.Types.Operation.Plan, options);
            if (await PickFolderAsync() is not { } parent) return;
            var folder = ScriptExporter.Export(options, parent, AppContext.BaseDirectory);
            Notice.Severity = InfoBarSeverity.Success; Notice.Title = "Build scripts exported"; Notice.Message = folder; Notice.IsOpen = true;
            OpenPath(folder);
        }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private void OpenPath(string path)
    {
        try { Process.Start(new ProcessStartInfo(path) { UseShellExecute = true }); }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private void Result_Click(object sender, RoutedEventArgs e) { if (File.Exists(_output)) OpenPath(Path.GetDirectoryName(_output)!); }
    private void Releases_Click(object sender, RoutedEventArgs e) => OpenPath(_engine switch
    {
        "lc0" => "https://github.com/LeelaChessZero/lc0/releases",
        "reckless" => "https://github.com/codedeliveryservice/Reckless/releases",
        "obsidian" => "https://github.com/gab8192/Obsidian/releases",
        "berserk" => "https://github.com/jhonnold/berserk/releases",
        _ => "https://github.com/official-stockfish/Stockfish/releases",
    });
    private void Networks_Click(object sender, RoutedEventArgs e) => OpenPath("https://lczero.org/play/networks/bestnets/");
    private void Log_Click(object sender, RoutedEventArgs e) { if (File.Exists(_logPath)) OpenPath(_logPath); }
    // Navigation (standard section 7, suite workspace): Compile, Downloads and Settings.
    private void Navigation_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args) =>
        ShowArea(args.IsSettingsSelected ? "settings" : (args.SelectedItemContainer?.Tag as string) ?? "compile");
    private void ShowArea(string area)
    {
        CompilePage.Visibility = area == "compile" ? Visibility.Visible : Visibility.Collapsed;
        DownloadPane.Visibility = area == "downloads" ? Visibility.Visible : Visibility.Collapsed;
        SettingsPage.Visibility = area == "settings" ? Visibility.Visible : Visibility.Collapsed;
        var item = area switch { "downloads" => DownloadsItem, "settings" => Navigation.SettingsItem, _ => (object)CompileItem };
        if (!ReferenceEquals(Navigation.SelectedItem, item)) Navigation.SelectedItem = item;
    }
    private void ShowCompile_Click(object sender, RoutedEventArgs e) => ShowArea("compile");
    private void ShowDownloads_Click(object sender, RoutedEventArgs e) => ShowArea("downloads");
    private void Settings_Click(object sender, RoutedEventArgs e) => ShowArea("settings");
    private void UpdateGitHubAccess() => GitHubAccessText.Text = _githubCredentials.Mode switch
    {
        "token" => "Personal access token",
        "cli" => "GitHub CLI account",
        _ => "Anonymous, without signing in",
    };
    private const string BerserkRequirements = "Install MSYS2, update it, then run in its UCRT64 terminal:\n\npacman -Syu\npacman -S --needed git make wget mingw-w64-ucrt-x86_64-gcc\n\nChoose the MSYS2 root folder, not its ucrt64 or bin folder. Berserk is built with UCRT64 GCC and a static runtime, so the result needs no MSYS2 DLLs.\n\nThe build downloads the network named by the selected source from jhonnold/berserk-networks, checks its SHA-256 against the file name and embeds it in berserk.exe. Source and network downloads need internet access. Every build has a new job folder; existing builds are kept.";
    private async void Requirements_Click(object sender, RoutedEventArgs e) => await new ContentDialog
    {
        XamlRoot = Root.XamlRoot,
        Title = "Build requirements",
        CloseButtonText = "Close",
        Content = new TextBlock { Text = _engine == "reckless" ? "Install Rust 1.88 or later with the x86_64-pc-windows-msvc target, Visual Studio x64 C++ build tools and Windows SDK, Git for Windows and curl. Cargo, rustc, git and curl must be on PATH. Restart the app after changing PATH. This adapter builds without Syzygy tablebases or PGO. Its source-selected network is embedded; each result passes UCI and a standalone benchmark. Tools are never installed by the app." : _engine == "obsidian" ? "Install MSYS2 UCRT64 GCC, git, make and curl. Choose the MSYS2 installation root. Obsidian embeds the network selected by its source into a static executable. The build records its SHA-256; upstream does not publish a digest for comparison. Regular and PGO builds are supported. Each result must pass UCI and a standalone benchmark. Existing jobs are preserved." : _engine == "berserk" ? BerserkRequirements : _engine == "lc0" ? "Install Git for Windows, Python 3, and Visual Studio with Desktop development with C++ and a Windows SDK. In the selected Python environment, run:\n\npython -m pip install meson ninja\n\nCPU (Eigen) needs no separate math library. NVIDIA GPU (CUDA) needs the CUDA Toolkit and a compatible driver. CUDA 12.x requires Visual Studio 2022. cuDNN is not required.\n\nChoose an Lc0 .pb.gz or .pb network from Help > Lc0 network downloads. A real search checks the selected network and backend. Large networks need more memory. The result includes the network and required CUDA DLLs; it still needs a compatible NVIDIA GPU/driver.\n\nTools are detected or selected; they are never installed by the app." : "Install MSYS2, update it, then run in its UCRT64 terminal:\n\npacman -Syu\npacman -S --needed git make mingw-w64-ucrt-x86_64-gcc\n\nChoose the MSYS2 root folder, not its ucrt64 or bin folder. Source and NNUE downloads need internet access. Every build has a new job folder; existing builds are kept.", TextWrapping = TextWrapping.Wrap }
    }.ShowAsync();
    private async void About_Click(object sender, RoutedEventArgs e) => await new ContentDialog
    {
        XamlRoot = Root.XamlRoot,
        Title = "About Chess Engine Compiler",
        CloseButtonText = "Close",
        Content = $"Chess Engine Compiler {System.Reflection.Assembly.GetExecutingAssembly().GetName().Version?.ToString(3)}\nPart of Jupiter · MIT license · Windows 11 x64\nBuild {System.Reflection.Assembly.GetExecutingAssembly().GetCustomAttributes(typeof(System.Reflection.AssemblyInformationalVersionAttribute), false).Cast<System.Reflection.AssemblyInformationalVersionAttribute>().FirstOrDefault()?.InformationalVersion}\n\nStockfish, Lc0, Berserk, Obsidian and Reckless are supported. Engines, networks and runtime libraries retain their upstream licenses."
    }.ShowAsync();
    private async void Exit_Click(object sender, RoutedEventArgs e) => await CloseAsync();
    private async Task CloseAsync()
    {
        if (_closing) return; _closing = true;
        if (_githubDialog is not null)
        {
            var pending = _githubDialogTask;
            _authSession?.Cancel(); _githubDialog.Hide();
            if (pending is not null) await pending;
        }
        if (_active || DownloadPane.DownloadActive)
        {
            var choice = await new ContentDialog { XamlRoot = Root.XamlRoot, Title = "Stop running work and close?", Content = "Closing will stop the running build or download. Build sources and logs remain. Incomplete download files are removed.", PrimaryButtonText = "Stop and close", CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Close }.ShowAsync();
            if (choice != ContentDialogResult.Primary) { _closing = false; return; }
        }
        SaveWindowBounds();
        _timer.Stop(); AppWindow.Hide();
        await _sourceSession.DisposeAsync();
        await DownloadPane.DisposeAsync();
        await Task.Run(_core.Dispose);
        _allowClose = true; Close();
    }
}

internal static partial class WindowDpi
{
    [System.Runtime.InteropServices.LibraryImport("user32.dll")]
    internal static partial uint GetDpiForWindow(nint hwnd);
}
