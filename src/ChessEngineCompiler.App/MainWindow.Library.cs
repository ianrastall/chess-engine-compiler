using Jupiter.ChessEngineCompiler.App.Services;
using Jupiter.ChessEngineCompiler.Messages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jupiter.ChessEngineCompiler.App;

public sealed partial class MainWindow
{
    private readonly GitHubSession _librarySession = new();
    private bool _checkingLibrary, _skipLibraryCheck, _libraryRunActive;
    private LibraryRow[] _libraryRows = [];
    private string CatalogRoot => Path.Combine(DataFolder, "release-library");

    public sealed record ReleaseItem(string Label, string State, LibraryRow Row);

    private BuildOptions LibraryOptions(string engine)
    {
        if (_engine == engine) return Options();
        if (_engineOptions.TryGetValue(engine, out var saved)) return saved.Clone();
        var options = _core.Call(Request.Types.Operation.Defaults, new BuildOptions { Engine = engine }).Options;
        var path = OptionsPath(engine);
        if (File.Exists(path)) options = ReadOptions(File.ReadAllText(path));
        options.Engine = engine;
        if (options.OutputRoot.Length == 0) options.OutputRoot = Model.OutputRoot;
        return options;
    }

    private LibraryQuery LibraryRequest()
    {
        var request = new LibraryQuery { CatalogRoot = CatalogRoot, Scope = (LibraryQuery.Types.Scope)LibraryScope.SelectedIndex };
        if (LibraryEngine.SelectedIndex != 2) request.Options.Add(LibraryOptions("stockfish"));
        if (LibraryEngine.SelectedIndex != 1) request.Options.Add(LibraryOptions("lc0"));
        return request;
    }

    private void PreviewLibrary()
    {
        try
        {
            SetLibraryRows(_core.Call(Request.Types.Operation.LibraryPreview, library: LibraryRequest()).Library);
        }
        catch (Exception error)
        {
            SetLibraryRows([]);
            LibraryStatus.Text = error.Message;
        }
    }

    private async Task CheckLibraryAsync()
    {
        if (_checkingLibrary || _closing) return;
        _checkingLibrary = true;
        _skipLibraryCheck = false;
        UpdateLibraryControls();
        var errors = new List<string>();
        try
        {
            foreach (var engine in new[] { "stockfish", "lc0" })
            {
                if (_skipLibraryCheck || _closing) break;
                LibraryStatus.Text = $"Checking {EngineName(engine)} releases…";
                try
                {
                    await _librarySession.RunAsync(_githubCredentials.Prepare(new GitHubQuery
                    {
                        Action = GitHubQuery.Types.Action.StableReleases,
                        Engine = engine,
                        OutputFolder = CatalogRoot,
                    }));
                }
                catch (OperationCanceledException) { break; }
                catch (Exception error) { errors.Add($"{EngineName(engine)}: {error.Message}"); }
            }
            if (_closing) return;
            LibraryStatus.Text = _skipLibraryCheck ? "Check skipped. Showing cached releases."
                : errors.Count > 0 ? string.Join(" ", errors) + " Showing available releases."
                : "Release check complete.";
            if (!_libraryRunActive) PreviewLibrary();
        }
        finally
        {
            _checkingLibrary = false;
            if (!_closing) UpdateLibraryControls();
        }
    }

    private async void CheckLibrary_Click(object sender, RoutedEventArgs e)
    {
        if (_checkingLibrary)
        {
            _skipLibraryCheck = true;
            _librarySession.Cancel();
            return;
        }
        await CheckLibraryAsync();
    }

    private void BuildLibrary_Click(object sender, RoutedEventArgs e)
    {
        if (_active || ToolSetupPane.SetupActive) return;
        try
        {
            _core.Call(Request.Types.Operation.LibraryStart, library: LibraryRequest());
            _libraryRunActive = true;
            _logPresenter.BeginBuild();
            Notice.IsOpen = false;
            Refresh();
        }
        catch (Exception error) { ShowProblem(error.Message); }
    }

    private void RefreshLibraryRun(Snapshot snapshot)
    {
        if (_libraryRunActive)
        {
            SetLibraryRows(snapshot.Library);
            if (snapshot.LogPath.Length > 0 && _logPath != snapshot.LogPath)
                _logPresenter.BeginBuild();
            if (!_active)
            {
                _libraryRunActive = false;
                LibraryStatus.Text = snapshot.Message;
            }
        }
        UpdateLibraryControls();
    }

    private void SetLibraryRows(IEnumerable<LibraryRow> rows)
    {
        var next = rows.ToArray();
        if (_libraryRows.SequenceEqual(next)) return;
        var previous = (LibraryResults.SelectedItem as ReleaseItem)?.Row;
        var items = next.Select(row => new ReleaseItem($"{EngineName(row.Engine)} {row.Tag}", row.State, row)).ToArray();
        _libraryRows = next;
        LibraryResults.ItemsSource = items;
        LibraryResults.SelectedItem = previous is null ? items.FirstOrDefault()
            : items.FirstOrDefault(item => item.Row.Engine == previous.Engine && item.Row.Tag == previous.Tag);
    }

    private void UpdateLibraryControls()
    {
        var idle = !_active && !ToolSetupPane.SetupActive;
        LibraryEngine.IsEnabled = LibraryScope.IsEnabled = idle;
        BuildLibrary.IsEnabled = idle && _libraryRows.Any(row => row.State is "Ready" or "Failed" or "Cancelled");
        BuildLibrary.Content = LibraryScope.SelectedIndex switch
        {
            1 => "Build latest releases",
            2 => "Build release history",
            _ => "Build new releases",
        };
        CheckLibrary.Content = _checkingLibrary ? "Skip check" : "Check releases";
        if (LibraryPane.Visibility == Visibility.Visible && CompilePage.Visibility == Visibility.Visible)
        {
            StartMenu.Text = (string)BuildLibrary.Content;
            StartMenu.IsEnabled = BuildLibrary.IsEnabled;
        }
    }

    private void LibrarySelection_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (_timer is null || _active) return;
        PreviewLibrary();
        UpdateLibraryControls();
    }

    private void LibraryRow_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (LibraryDetail is null) return;
        var row = (LibraryResults.SelectedItem as ReleaseItem)?.Row;
        LibraryDetail.Text = row?.Message ?? "";
        LibraryResultButton.IsEnabled = File.Exists(row?.Output);
        LibraryLogButton.IsEnabled = File.Exists(row?.LogPath);
    }

    private void LibraryResult_Click(object sender, RoutedEventArgs e)
    {
        if (LibraryResults.SelectedItem is ReleaseItem item && File.Exists(item.Row.Output))
            OpenPath(Path.GetDirectoryName(item.Row.Output)!);
    }

    private void LibraryLog_Click(object sender, RoutedEventArgs e)
    {
        if (LibraryResults.SelectedItem is ReleaseItem item && File.Exists(item.Row.LogPath))
            OpenPath(item.Row.LogPath);
    }

    private void CheckOnStartup_Click(object sender, RoutedEventArgs e)
    {
        _preferences.CheckReleasesOnStartup = CheckOnStartup.IsChecked == true;
        _preferences.Save();
    }
}
