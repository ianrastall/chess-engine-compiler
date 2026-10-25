using System.Diagnostics;
using Jupiter.ChessEngineCompiler.Interop;
using Jupiter.ChessEngineCompiler.App.Services;
using Jupiter.ChessEngineCompiler.Messages;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jupiter.ChessEngineCompiler.App.Views;

public sealed partial class ToolSetupView : UserControl, IAsyncDisposable
{
    private readonly CompilerCore _core = new();
    private readonly LogPresenter _logPresenter;
    private readonly DispatcherQueueTimer _timer;
    private ToolSetupOptions _options = new();
    private bool _verification, _reading, _disposed, _buildActive;
    private string _logPath = "", _startedTool = "", _lastState = "Ready";
    public bool SetupActive { get; private set; }
    public string CurrentPhase { get; private set; } = "";
    public event Action<string>? ToolInstalled;

    public ToolSetupView()
    {
        InitializeComponent();
        _logPresenter = new LogPresenter(SetupScroll, SetupLog, FollowSetupOutput, SetupLogStatus);
        _timer = DispatcherQueue.CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(400);
        _timer.Tick += (_, _) => Poll();
        _timer.Start();
    }

    internal void Configure(BuildOptions o, string destination, bool verification)
    {
        if (SetupActive || _reading) return;
        _verification = verification;
        _options = new ToolSetupOptions { MsysRoot = o.MsysRoot, PythonPath = o.PythonPath, DestinationRoot = destination };
        EngineRequirements.Text = o.Engine switch
        {
            "lc0" => "Lc0 requires Git, Python with Meson/Ninja, and Microsoft C++ build tools. CUDA additionally needs the NVIDIA toolkit and a compatible driver.",
            "reckless" => "Reckless requires Rust/Cargo, Microsoft C++ build tools, Git and curl.",
            "avalanche" => "Avalanche requires Git and Zig 0.17.0. The private Zig installation can be used immediately without restarting.",
            _ => "The selected engine requires MSYS2 with UCRT64 GCC, make, Git and download tools.",
        };
        SelectedPaths.Text = $"MSYS2 root: {(o.MsysRoot.Length > 0 ? o.MsysRoot : "select it in the Compile form")}\nPython: {(o.PythonPath.Length > 0 ? o.PythonPath : "detected on PATH")}";
        var preferred = o.Engine switch { "avalanche" => "zig", "reckless" => "rust", "lc0" => "meson", _ => "gcc" };
        _ = RefreshCatalogAsync(preferred);
    }

    internal void SetBuildActive(bool value) { _buildActive = value; UpdateButtons(); }

    private async Task RefreshCatalogAsync(string? preferred = null)
    {
        if (_reading || SetupActive || _disposed) return;
        _reading = true;
        if (_lastState == "Ready") SetupStatus.Text = "Checking installed tools...";
        UpdateButtons();
        try
        {
            var selected = preferred ?? (ToolChoice.SelectedItem as BuildTool)?.Id;
            var options = _options.Clone();
            var result = await Task.Run(() => _core.Call(Request.Types.Operation.ToolCatalog, toolSetup: options));
            if (_disposed) return;
            ToolChoice.ItemsSource = result.Tools.OrderBy(t => t.Name, StringComparer.OrdinalIgnoreCase).ToArray();
            ToolChoice.SelectedItem = result.Tools.FirstOrDefault(t => t.Id == selected);
            if (ToolChoice.SelectedIndex < 0) ToolChoice.SelectedIndex = 0;
            ShowSelection();
            if (_lastState == "Ready") SetupStatus.Text = "Tool check completed. Select a tool to review setup.";
        }
        catch (Exception e) { SetupStatus.Text = e.Message; }
        finally { _reading = false; if (!_disposed) UpdateButtons(); }
    }

    private void Tool_Changed(object sender, SelectionChangedEventArgs e) => ShowSelection();

    private void ShowSelection()
    {
        if (ToolChoice?.SelectedItem is BuildTool tool) { DetectedStatus.Text = tool.Status; Details.Text = tool.Details; }
        if (InstallTool is not null) UpdateButtons();
    }

    private void UpdateButtons()
    {
        var tool = ToolChoice.SelectedItem as BuildTool;
        InstallTool.IsEnabled = !_disposed && !_verification && !_reading && !_buildActive && !SetupActive && tool?.InstallAvailable == true;
        CheckTools.IsEnabled = !_disposed && !_reading && !SetupActive;
        ToolChoice.IsEnabled = !_reading && !SetupActive;
        OfficialDownload.IsEnabled = !SetupActive && tool is not null;
        CancelSetup.IsEnabled = SetupActive;
    }

    private void Install_Click(object sender, RoutedEventArgs e)
    {
        if (!InstallTool.IsEnabled || ToolChoice.SelectedItem is not BuildTool tool) return;
        try
        {
            var options = _options.Clone();
            options.ToolId = tool.Id;
            _core.Call(Request.Types.Operation.ToolSetupStart, toolSetup: options);
            _startedTool = tool.Id;
            SetupActive = true;
            _lastState = "Running";
            SetupStatus.Text = "Running - review any installer prompts.";
            _logPresenter.BeginBuild();
            UpdateButtons();
            Poll();
        }
        catch (Exception error) { SetupStatus.Text = error.Message; }
    }

    private async void Check_Click(object sender, RoutedEventArgs e) => await RefreshCatalogAsync();

    private void Download_Click(object sender, RoutedEventArgs e)
    {
        try { if (ToolChoice.SelectedItem is BuildTool tool) Process.Start(new ProcessStartInfo(tool.Url) { UseShellExecute = true }); }
        catch (Exception error) { SetupStatus.Text = error.Message; }
    }

    private void Cancel_Click(object sender, RoutedEventArgs e)
    {
        try { _core.Call(Request.Types.Operation.Cancel); Poll(); }
        catch (Exception error) { SetupStatus.Text = error.Message; }
    }

    private void Log_Click(object sender, RoutedEventArgs e)
    {
        try { if (File.Exists(_logPath)) Process.Start(new ProcessStartInfo(_logPath) { UseShellExecute = true }); }
        catch (Exception error) { SetupStatus.Text = error.Message; }
    }

    private void Poll()
    {
        if (_disposed || _reading || !SetupActive) return;
        try
        {
            var s = _core.Call(Request.Types.Operation.Status).Snapshot;
            SetupActive = s.State is "Running" or "Cancelling";
            CurrentPhase = s.Phase;
            SetupStatus.Text = $"{s.State} - {s.Phase}\n{s.Message}";
            _logPresenter.Update(s.LogTail);
            _logPath = s.LogPath;
            OpenSetupLog.IsEnabled = File.Exists(_logPath);
            SetupProgress.IsIndeterminate = SetupActive;
            SetupProgress.Visibility = SetupActive ? Visibility.Visible : Visibility.Collapsed;
            if (!SetupActive && _lastState is "Running" or "Cancelling")
            {
                if (s.State == "Completed") ToolInstalled?.Invoke(_startedTool);
                _startedTool = "";
                _ = RefreshCatalogAsync();
            }
            _lastState = s.State;
            UpdateButtons();
        }
        catch (Exception error) { SetupStatus.Text = error.Message; SetupActive = false; UpdateButtons(); }
    }

    internal async Task VerifyControlsAsync()
    {
        while (_reading) await Task.Delay(20);
        if (!_verification || ToolChoice.Items.Count < 8) throw new InvalidOperationException("Build-tool catalog did not load.");
        foreach (var id in new[] { "zig", "rust", "python", "gcc", "meson", "msvc", "cuda" })
        {
            ToolChoice.SelectedItem = ToolChoice.Items.Cast<BuildTool>().Single(t => t.Id == id);
            if (InstallTool.IsEnabled || !Details.Text.Any() || !((BuildTool)ToolChoice.SelectedItem).Url.StartsWith("https://")) throw new InvalidOperationException("Tool setup review or safe verification state failed.");
        }
        ToolChoice.SelectedItem = ToolChoice.Items.Cast<BuildTool>().Single(t => t.Id == "zig");
        _verification = false; SetBuildActive(false);
        if (!InstallTool.IsEnabled) throw new InvalidOperationException("An available Zig installation was not enabled after review.");
        SetBuildActive(true);
        if (InstallTool.IsEnabled) throw new InvalidOperationException("Tool installation was enabled while compiling.");
        SetBuildActive(false); SetupActive = true; UpdateButtons();
        if (InstallTool.IsEnabled || !CancelSetup.IsEnabled) throw new InvalidOperationException("Running setup controls are wrong.");
        SetupActive = false; _verification = true; UpdateButtons();
        if (!Details.Text.Contains("SHA-256")) throw new InvalidOperationException("Zig setup integrity details are missing.");
    }

    public async ValueTask DisposeAsync()
    {
        if (_disposed) return;
        _disposed = true;
        _timer.Stop();
        while (_reading) await Task.Delay(20);
        if (SetupActive) _core.Call(Request.Types.Operation.Cancel);
        await Task.Run(_core.Dispose);
    }
}
