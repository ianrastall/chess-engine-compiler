using System.Diagnostics;
using System.Text.Json;
using Jupiter.ChessEngineCompiler.App.Services;
using Jupiter.ChessEngineCompiler.Interop;
using Jupiter.ChessEngineCompiler.Messages;
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
    private bool _active, _closing, _allowClose;
    private string _output = "", _logPath = "", _lastState = "Ready";
    private static readonly string DataFolder = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Jupiter", "ChessEngineCompiler");
    private static string ConfigPath => Path.Combine(DataFolder, "build-options.json");
    public BuildViewModel Model { get; } = new();
    public MainWindow()
    {
        InitializeComponent();
        Title = "Chess Engine Compiler";
        Directory.CreateDirectory(DataFolder);
        _accent = new AccentService(ShellPreferences.Load(Path.Combine(DataFolder, "ui.json")), DispatcherQueue);
        _accent.Attach(Root);
        var options = _core.Call(Request.Types.Operation.Defaults).Options;
        options.OutputRoot = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments), "Chess engine builds");
        try { if (File.Exists(ConfigPath)) options = ReadOptions(File.ReadAllText(ConfigPath)); }
        catch (Exception e) when (e is IOException or JsonException or InvalidOperationException) { ShowProblem("Saved options could not be read. Defaults have been restored."); }
        LoadOptions(options);
        _timer = DispatcherQueue.CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(400);
        _timer.Tick += (_, _) => Refresh();
        _timer.Start();
        AppWindow.Closing += (window, e) => { if (!_allowClose) { e.Cancel = true; _ = CloseAsync(); } };
    }
    public void ConfigureWindow()
    {
        var scale = WindowDpi.GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        var area = Microsoft.UI.Windowing.DisplayArea.GetFromWindowId(AppWindow.Id, Microsoft.UI.Windowing.DisplayAreaFallback.Primary).WorkArea;
        if (AppWindow.Presenter is Microsoft.UI.Windowing.OverlappedPresenter presenter)
        {
            presenter.PreferredMinimumWidth = Math.Min((int)(1050 * scale), area.Width);
            presenter.PreferredMinimumHeight = Math.Min((int)(650 * scale), area.Height);
        }
        var width = Math.Min((int)(1200 * scale), area.Width);
        var height = Math.Min((int)(900 * scale), area.Height);
        AppWindow.MoveAndResize(new Windows.Graphics.RectInt32(area.X + (area.Width - width) / 2, area.Y + (area.Height - height) / 2, width, height));
    }
    private void LoadOptions(BuildOptions options)
    {
        Model.SourceRef = options.SourceRef; Model.MsysRoot = options.MsysRoot; Model.OutputRoot = options.OutputRoot;
        Jobs.Value = options.Jobs; Pgo.IsChecked = options.Pgo; Strip.IsChecked = options.Strip;
        foreach (ComboBoxItem item in Architecture.Items) if ((string)item.Content == options.Architecture) Architecture.SelectedItem = item;
    }
    private BuildOptions Options() => new()
    {
        Engine = "stockfish",
        SourceRef = Model.SourceRef.Trim(),
        MsysRoot = Model.MsysRoot.Trim(),
        OutputRoot = Model.OutputRoot.Trim(),
        Architecture = (string)((ComboBoxItem)Architecture.SelectedItem).Content,
        Jobs = double.IsFinite(Jobs.Value) ? (uint)Math.Clamp(Jobs.Value, 0, 256) : 0,
        Pgo = Pgo.IsChecked == true,
        Strip = Strip.IsChecked == true,
    };
    private static BuildOptions ReadOptions(string json) => Google.Protobuf.JsonParser.Default.Parse<BuildOptions>(json);
    private void ShowProblem(string message) { Notice.Severity = InfoBarSeverity.Error; Notice.Title = "Build needs attention"; Notice.Message = message; Notice.IsOpen = true; }
    private void Preview_Click(object sender, RoutedEventArgs e)
    {
        try { Log.Text = "Build preview\n\n" + string.Join("\n\n", _core.Call(Request.Types.Operation.Plan, Options()).Steps); Notice.IsOpen = false; }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private void Start_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            var options = Options();
            _core.Call(Request.Types.Operation.Start, options);
            Notice.IsOpen = false;
            Refresh();
            try { File.WriteAllText(ConfigPath, ScriptExporter.SerializeOptions(options)); }
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
            if ((_active || _lastState is "Running" or "Cancelling") && Log.Text != s.LogTail) Log.Text = s.LogTail;
            _output = s.Output; _logPath = s.LogPath;
            ResultButton.IsEnabled = File.Exists(_output); LogButton.IsEnabled = File.Exists(_logPath);
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
    private void Detect_Click(object sender, RoutedEventArgs e)
    {
        if (_active) return;
        try { Model.MsysRoot = _core.Call(Request.Types.Operation.Defaults).Options.MsysRoot; if (Model.MsysRoot.Length == 0) ShowProblem("MSYS2 UCRT64 was not found. Browse to its root folder or see Help > Build requirements."); }
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
    private void Releases_Click(object sender, RoutedEventArgs e) => OpenPath("https://github.com/official-stockfish/Stockfish/releases");
    private void Log_Click(object sender, RoutedEventArgs e) { if (File.Exists(_logPath)) OpenPath(_logPath); }
    private async void Appearance_Click(object sender, RoutedEventArgs e)
    {
        var useWindows = new RadioButton { Content = "Use Windows accent color", IsChecked = !_accent.UseCustomAccent };
        var custom = new RadioButton { Content = "Custom color", IsChecked = _accent.UseCustomAccent };
        var picker = new ColorPicker { Color = _accent.CustomColor, Width = 280, IsAlphaEnabled = false, IsAlphaSliderVisible = false, IsAlphaTextInputVisible = false, IsMoreButtonVisible = false, IsColorSliderVisible = false, IsColorChannelTextInputVisible = false };
        picker.IsEnabled = custom.IsChecked == true;
        useWindows.Checked += (_, _) => { _accent.UseWindows(); picker.IsEnabled = false; };
        custom.Checked += (_, _) => { picker.IsEnabled = true; _accent.UseCustom(picker.Color); };
        picker.ColorChanged += (_, _) => { if (custom.IsChecked == true) _accent.UseCustom(picker.Color); };
        var reset = new Button { Content = "Reset to Windows accent" };
        reset.Click += (_, _) => { useWindows.IsChecked = true; _accent.UseWindows(); };
        var panel = new StackPanel { Spacing = 8 }; panel.Children.Add(new TextBlock { Text = "Accent color" }); panel.Children.Add(useWindows); panel.Children.Add(custom); panel.Children.Add(picker); panel.Children.Add(reset);
        await new ContentDialog { XamlRoot = Root.XamlRoot, Title = "Appearance", Content = panel, CloseButtonText = "Close" }.ShowAsync();
    }
    private async void Requirements_Click(object sender, RoutedEventArgs e) => await new ContentDialog
    {
        XamlRoot = Root.XamlRoot,
        Title = "Build requirements",
        CloseButtonText = "Close",
        Content = new TextBlock { Text = "Install MSYS2, update it, then run in its UCRT64 terminal:\n\npacman -Syu\npacman -S --needed git make mingw-w64-ucrt-x86_64-gcc\n\nChoose the MSYS2 root folder, not its ucrt64 or bin folder. Source and NNUE downloads need internet access. Every build has a new job folder; existing builds are kept.", TextWrapping = TextWrapping.Wrap }
    }.ShowAsync();
    private async void About_Click(object sender, RoutedEventArgs e) => await new ContentDialog
    {
        XamlRoot = Root.XamlRoot,
        Title = "Chess Engine Compiler",
        CloseButtonText = "Close",
        Content = "Jupiter · Chess Engine Compiler\nVersion 0.1.0 · Windows 11 x64\nMIT license\n\nStockfish is the first supported engine. Lc0 is next.\nStockfish and its networks retain their upstream licenses."
    }.ShowAsync();
    private async void Exit_Click(object sender, RoutedEventArgs e) => await CloseAsync();
    private async Task CloseAsync()
    {
        if (_closing) return; _closing = true;
        if (_active)
        {
            var choice = await new ContentDialog { XamlRoot = Root.XamlRoot, Title = "Cancel the running build?", Content = "Closing will stop compilation. Its source and log will remain in the job folder.", PrimaryButtonText = "Cancel build and close", CloseButtonText = "Keep building", DefaultButton = ContentDialogButton.Close }.ShowAsync();
            if (choice != ContentDialogResult.Primary) { _closing = false; return; }
        }
        _timer.Stop(); AppWindow.Hide();
        await Task.Run(_core.Dispose);
        _allowClose = true; Close();
    }
}

internal static partial class WindowDpi
{
    [System.Runtime.InteropServices.LibraryImport("user32.dll")]
    internal static partial uint GetDpiForWindow(nint hwnd);
}
