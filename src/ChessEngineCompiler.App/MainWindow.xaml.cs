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
    private bool _downloadingWeights;
    private readonly Dictionary<string, BuildOptions> _engineOptions = new();
    private readonly bool _verification;
    private readonly GitHubCredentials _githubCredentials;
    private readonly GitHubSession _sourceSession = new();
    private Hardware _hardware = new();
    private readonly string DataFolder;
    private string ConfigPath => Path.Combine(DataFolder, "build-options.json");
    private string WeightsFolder => Path.Combine(DataFolder, "networks");
    private static readonly string[] Engines = EngineCatalog.Items.Select(e => e.Id).ToArray();
    private static string EngineName(string engine) => EngineCatalog.Name(engine);
    private string OptionsPath(string engine) => engine == "stockfish" ? ConfigPath : Path.Combine(DataFolder, $"{engine}-options.json");
    private string RecommendedArchitecture => _engine == "obsidian" ? _hardware.ObsidianRecommended : _hardware.Recommended;
    private IEnumerable<string> ArchitectureChoices => _engine == "obsidian" ? _hardware.ObsidianChoices : _hardware.Choices;
    public BuildViewModel Model { get; } = new();
    public MainWindow(string? dataFolder = null)
    {
        DataFolder = dataFolder ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Jupiter", "ChessEngineCompiler");
        _verification = dataFolder is not null;
        InitializeComponent();
        Engine.ItemsSource = EngineCatalog.Items;
        LibraryEngine.ItemsSource = new[] { new EngineEntry("", "All engines") }.Concat(EngineCatalog.Items).ToArray();
        LibraryEngine.SelectedIndex = 0;
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
            if (last != "stockfish" && Engines.Contains(last) && File.Exists(OptionsPath(last))) options = ReadOptions(File.ReadAllText(OptionsPath(last)));
        }
        catch (Exception e) when (e is IOException or JsonException or InvalidOperationException) { ShowProblem("Saved engine options could not be read. Stockfish settings have been restored."); }
        LoadOptions(options);
        ToolSetupPane.ToolInstalled += id =>
        {
            if (id is "msys2" or "gcc" && string.IsNullOrWhiteSpace(Model.MsysRoot)) Model.MsysRoot = _core.Call(Request.Types.Operation.Defaults).Options.MsysRoot;
            if (id == "meson" && _engine == "lc0") PythonPath.Text = _core.Call(Request.Types.Operation.Defaults, new BuildOptions { Engine = "lc0" }).Options.PythonPath;
        };
        Engine.SelectionChanged += Engine_Changed;
        _timer = DispatcherQueue.CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(400);
        _timer.Tick += (_, _) => Refresh();
        _timer.Start();
        CheckOnStartup.IsChecked = _preferences.CheckReleasesOnStartup;
        ShowArea("library");
        PreviewLibrary();
        UpdateLibraryControls();
        if (!_verification && _preferences.CheckReleasesOnStartup) _ = CheckLibraryAsync();
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
        var lc0 = _engine == "lc0";
        Jobs.Value = options.Jobs; Pgo.IsChecked = options.Pgo && !lc0; Strip.IsChecked = options.Strip;
        AutoArchitecture.IsChecked = options.AutoArchitecture;
        foreach (ComboBoxItem item in Architecture.Items) if ((string)item.Content == options.Architecture) Architecture.SelectedItem = item;
        PythonPath.Text = options.PythonPath; VisualStudio.Text = options.VisualStudio;
        CudaRoot.Text = options.CudaRoot; WeightsPath.Text = options.WeightsPath;
        Backend.SelectedIndex = options.Backend == "eigen" ? 1 : 0;
        CpuBuildOptions.Visibility = MsysOptions.Visibility = Pgo.Visibility = AutoArchitecture.Visibility =
            lc0 ? Visibility.Collapsed : Visibility.Visible;
        Lc0Options.Visibility = lc0 ? Visibility.Visible : Visibility.Collapsed;
        SourceRef.IsReadOnly = SourceChannel.SelectedIndex != 2;
        CompileButton.Content = StartMenu.Text = $"Compile {EngineName(_engine)}";
        SourceHint.Text = _engine switch
        {
            "lc0" => "Default: Lc0 v0.32.1. Choose your network below. The release library can supply a verified default network.",
            "obsidian" => "Default: Obsidian v16.0. The source selects the network to embed.",
            _ => "Default: Stockfish 19 (sf_19). Use master for development source, or enter a tag or commit.",
        };
        ToolTipService.SetToolTip(SourceRef, $"Select a {EngineName(_engine)} release tag, branch or exact commit.");
        _loadingOptions = false;
        AutoArchitecture_Changed(this, null!);
        Backend_Changed(this, null!);
    }
    private void ShowArchitectures()
    {
        Architecture.Items.Clear();
        foreach (var choice in ArchitectureChoices) Architecture.Items.Add(new ComboBoxItem { Content = choice });
        Architecture.SelectedIndex = Architecture.Items.Count > 0 ? 0 : -1;
        ArchitectureExplanation.Text = _engine == "obsidian"
            ? $"Recommended Obsidian target: {_hardware.ObsidianRecommended}."
            : _hardware.Explanation;
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
                if (engine == "obsidian" && next.MsysRoot.Length == 0)
                    next.MsysRoot = _engineOptions["stockfish"].MsysRoot;
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
        Architecture = (Architecture.SelectedItem as ComboBoxItem)?.Content as string ?? "",
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
        ShowArea("compile");
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
        if (LibraryPane.Visibility == Visibility.Visible) { BuildLibrary_Click(sender, e); return; }
        ShowArea("compile");
        if (ToolSetupPane.SetupActive) { ShowProblem("Finish or cancel build-tool setup before compiling."); return; }
        try
        {
            var options = Options();
            _core.Call(Request.Types.Operation.Start, options);
            _logPresenter.BeginBuild();
            Notice.IsOpen = false;
            Refresh();
            try
            {
                SaveBuildOptions(options);
            }
            catch (IOException error) { ShowProblem($"Build started, but its options could not be saved: {error.Message}"); }
        }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private void SaveBuildOptions(BuildOptions options)
    {
        File.WriteAllText(OptionsPath(options.Engine), ScriptExporter.SerializeOptions(options));
        File.WriteAllText(Path.Combine(DataFolder, "last-engine.txt"), options.Engine);
        _engineOptions[options.Engine] = options.Clone();
    }

    private void SaveOptions_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            SaveBuildOptions(Options());
            Notice.Severity = InfoBarSeverity.Success;
            Notice.Title = "Settings saved";
            Notice.Message = "The release library will use these tool paths and output settings.";
            Notice.IsOpen = true;
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
            Configuration.IsEnabled = CompileButton.IsEnabled = StartMenu.IsEnabled = PreviewButton.IsEnabled = PreviewMenu.IsEnabled = SaveOptionsButton.IsEnabled = !_active && !ToolSetupPane.SetupActive;
            ToolSetupPane.SetBuildActive(_active);
            if (ToolSetupPane.SetupActive) Model.Status = $"Running - {ToolSetupPane.CurrentPhase}";
            CancelButton.IsEnabled = CancelMenu.IsEnabled = s.State == "Running";
            CancelButton.Content = CancelMenu.Text = _active && _downloadingWeights ? "Cancel download" : "Cancel build";
            Progress.IsIndeterminate = _active; Progress.Visibility = _active ? Visibility.Visible : Visibility.Collapsed;
            RefreshLibraryRun(s);
            if (_active || _downloadingWeights || _lastState is "Running" or "Cancelling") _logPresenter.Update(s.LogTail);
            _output = s.Output; _logPath = s.LogPath;
            ResultButton.IsEnabled = File.Exists(_output); LogButton.IsEnabled = File.Exists(_logPath);
            PowerShellLogMenu.IsEnabled = File.Exists(_logPath);
            var weightsFinished = _downloadingWeights && !_active;
            if (weightsFinished)
            {
                _downloadingWeights = false;
                if (s.State == "Completed")
                {
                    WeightsPath.Text = s.Output;
                    PreviewLibrary();
                    UpdateLibraryControls();
                }
            }
            if ((weightsFinished || _lastState != s.State) && s.State is "Completed" or "Failed" or "Cancelled")
            {
                Notice.Severity = s.State == "Completed" ? InfoBarSeverity.Success : s.State == "Failed" ? InfoBarSeverity.Error : InfoBarSeverity.Informational;
                Notice.Title = s.State; Notice.Message = s.Message; Notice.IsOpen = true;
                if (weightsFinished && s.State == "Completed") Notice.Message += " Select Save settings to keep this selection.";
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
            var picker = new FileOpenPicker { SettingsIdentifier = "Lc0Weights" }; picker.FileTypeFilter.Add(".gz"); picker.FileTypeFilter.Add(".pb");
            WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(this));
            if (await picker.PickSingleFileAsync() is { } file) WeightsPath.Text = file.Path;
        }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private void ManageWeights_Click(object sender, RoutedEventArgs e)
    {
        if (_active || ToolSetupPane.SetupActive) return;
        Engine.SelectedIndex = EngineCatalog.Index("lc0");
        ShowArea("compile");
        Root.UpdateLayout();
        OptionsScroll.ChangeView(null, OptionsScroll.ScrollableHeight, null, true);
    }
    private void OpenWeightsFolder_Click(object sender, RoutedEventArgs e)
    {
        try { OpenPath(Directory.CreateDirectory(WeightsFolder).FullName); }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private void DownloadWeights_Click(object sender, RoutedEventArgs e)
    {
        if (_active || ToolSetupPane.SetupActive) return;
        try
        {
            _core.Call(Request.Types.Operation.Lc0WeightsDownload, weightsFolder: WeightsFolder);
            _downloadingWeights = true;
            _logPresenter.BeginBuild();
            Notice.IsOpen = false;
            Refresh();
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
        "obsidian" => "https://github.com/gab8192/Obsidian/releases",
        _ => "https://github.com/official-stockfish/Stockfish/releases",
    });
    private void Networks_Click(object sender, RoutedEventArgs e) => OpenPath("https://lczero.org/play/networks/bestnets/");
    private void Log_Click(object sender, RoutedEventArgs e) { if (File.Exists(_logPath)) OpenPath(_logPath); }
    private void Navigation_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (_timer is not null)
            ShowArea(args.IsSettingsSelected ? "settings" : (args.SelectedItemContainer?.Tag as string) ?? "library");
    }

    private void ShowArea(string area)
    {
        var wasAdvanced = CompilePage.Visibility == Visibility.Visible && OptionsScroll.Visibility == Visibility.Visible;
        var library = area == "library";
        var advanced = area == "compile";
        CompilePage.Visibility = library || advanced ? Visibility.Visible : Visibility.Collapsed;
        LibraryPane.Visibility = LibraryActions.Visibility = library ? Visibility.Visible : Visibility.Collapsed;
        OptionsScroll.Visibility = AdvancedActions.Visibility = advanced ? Visibility.Visible : Visibility.Collapsed;
        DownloadPane.Visibility = area == "downloads" ? Visibility.Visible : Visibility.Collapsed;
        if (area == "settings") ToolSetupPane.Configure(Options(), _verification ? Path.Combine(DataFolder, "tools") : "", _verification);
        SettingsPage.Visibility = area == "settings" ? Visibility.Visible : Visibility.Collapsed;
        var item = area switch
        {
            "compile" => CompileItem,
            "downloads" => DownloadsItem,
            "settings" => Navigation.SettingsItem,
            _ => (object)LibraryItem,
        };
        if (!ReferenceEquals(Navigation.SelectedItem, item)) Navigation.SelectedItem = item;
        if (library)
        {
            if (wasAdvanced && !_active) PreviewLibrary();
            UpdateLibraryControls();
        }
        if (advanced)
        {
            StartMenu.Text = $"Compile {EngineName(_engine)}";
            if (!wasAdvanced && !_verification) _ = LoadSourcesAsync(1);
        }
    }

    private void ShowLibrary_Click(object sender, RoutedEventArgs e) => ShowArea("library");
    private void ShowCompile_Click(object sender, RoutedEventArgs e) => ShowArea("compile");
    private void ShowDownloads_Click(object sender, RoutedEventArgs e) => ShowArea("downloads");
    private void Tools_Click(object sender, RoutedEventArgs e) { ShowArea("settings"); DispatcherQueue.TryEnqueue(() => ToolSetupPane.StartBringIntoView()); }
    private void Settings_Click(object sender, RoutedEventArgs e) => ShowArea("settings");
    private void UpdateGitHubAccess() => GitHubAccessText.Text = _githubCredentials.Mode switch
    {
        "token" => "Personal access token",
        "cli" => "GitHub CLI account",
        _ => "Anonymous, without signing in",
    };
    private async void Requirements_Click(object sender, RoutedEventArgs e) => await new ContentDialog
    {
        XamlRoot = Root.XamlRoot,
        Title = "Build requirements",
        CloseButtonText = "Close",
        Content = new TextBlock
        {
            Text = _engine == "lc0"
                ? "Lc0 requires Git, Python with Meson and Ninja, and the Visual Studio x64 C++ tools and Windows SDK. NVIDIA GPU builds also need a compatible CUDA Toolkit and driver. Use Settings > Build tools for setup. The release library can supply a verified default network."
                : $"{EngineName(_engine)} requires Git and MSYS2 UCRT64 GCC, make and download tools. Select the MSYS2 installation root. Use Settings > Build tools to check and install prerequisites.",
            TextWrapping = TextWrapping.Wrap,
        },
    }.ShowAsync();

    private async void About_Click(object sender, RoutedEventArgs e) => await new ContentDialog
    {
        XamlRoot = Root.XamlRoot,
        Title = "About Chess Engine Compiler",
        CloseButtonText = "Close",
        Content = $"Chess Engine Compiler {System.Reflection.Assembly.GetExecutingAssembly().GetName().Version?.ToString(3)}\nPart of Jupiter · MIT license · Windows 11 x64\nBuild {System.Reflection.Assembly.GetExecutingAssembly().GetCustomAttributes(typeof(System.Reflection.AssemblyInformationalVersionAttribute), false).Cast<System.Reflection.AssemblyInformationalVersionAttribute>().FirstOrDefault()?.InformationalVersion}\n\nSupported engines: {string.Join(", ", EngineCatalog.Items.Select(e => e.Name))}. Engines, networks and runtime libraries retain their upstream licenses."
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
        if (_active || DownloadPane.DownloadActive || ToolSetupPane.SetupActive)
        {
            var choice = await new ContentDialog { XamlRoot = Root.XamlRoot, Title = "Stop running work and close?", Content = "Closing will stop running builds, downloads or tool setup. Logs and source remain. Partial tool installations may remain; check their installers before retrying. Incomplete downloads are removed.", PrimaryButtonText = "Stop and close", CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Close }.ShowAsync();
            if (choice != ContentDialogResult.Primary) { _closing = false; return; }
        }
        SaveWindowBounds();
        _timer.Stop(); AppWindow.Hide();
        await _sourceSession.DisposeAsync();
        await _librarySession.DisposeAsync();
        await DownloadPane.DisposeAsync();
        await ToolSetupPane.DisposeAsync();
        await Task.Run(_core.Dispose);
        _allowClose = true; Close();
    }
}

internal static partial class WindowDpi
{
    [System.Runtime.InteropServices.LibraryImport("user32.dll")]
    internal static partial uint GetDpiForWindow(nint hwnd);
}
