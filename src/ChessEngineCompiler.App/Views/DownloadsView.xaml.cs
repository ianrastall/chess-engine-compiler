using System.Diagnostics;
using Jupiter.ChessEngineCompiler.App.Services;
using Jupiter.ChessEngineCompiler.Messages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Storage.Pickers;

namespace Jupiter.ChessEngineCompiler.App.Views;

public sealed partial class DownloadsView : UserControl, IAsyncDisposable
{
    private readonly GitHubSession _browse = new(), _download = new();
    private GitHubCredentials? _auth;
    private bool _verification, _loaded, _loading, _disposed;
    private uint _page = 1, _catalogPage = 1;
    private string _output = "";
    private nint _owner;
    private string Engine => DownloadEngine.SelectedIndex switch { 1 => "lc0", 2 => "berserk", 3 => "obsidian", _ => "stockfish" };
    public bool DownloadActive => _download.Active;
    public DownloadsView()
    {
        InitializeComponent();
        Folder.Text = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments), "Chess engine downloads");
        Loaded += (_, _) => { if (!_loaded) { _loaded = true; if (!_verification && _auth is not null) _ = LoadCatalogAsync(1); } };
    }
    public void Initialize(GitHubCredentials auth, bool verification, nint owner) { _auth = auth; _verification = verification; _owner = owner; RefreshAuthentication(); }
    public void RefreshAuthentication()
    {
        if (_auth is null) return;
        AuthenticationStatus.Text = _auth.Connected ? $"GitHub access: {_auth.Mode}. Workflow downloads depend on the account’s Actions permissions." : "Release assets use anonymous access. Connect an account or token in Settings > GitHub for workflow artifacts and higher API limits.";
    }
    private void Problem(string message) { Notice.Severity = InfoBarSeverity.Error; Notice.Title = "GitHub downloads"; Notice.Message = message; Notice.IsOpen = true; }
    private async Task LoadCatalogAsync(uint page)
    {
        if (_auth is null || _browse.Active || _disposed) return;
        var engine = Engine;
        CatalogRefresh.IsEnabled = CatalogNewer.IsEnabled = CatalogOlder.IsEnabled = false;
        try
        {
            var result = await _browse.RunAsync(_auth.Prepare(new GitHubQuery { Engine = engine, Page = page, Action = GitHubQuery.Types.Action.Catalog }));
            if (_disposed || engine != Engine) return;
            _catalogPage = page; _loading = true; Release.ItemsSource = result.Releases.ToList(); Release.SelectedIndex = result.Releases.Count > 0 ? 0 : -1; _loading = false;
            CatalogStatus.Text = $"Release page {page}. Choose the file matching your system and CPU/GPU; assets may also include source or checksum files.";
            CatalogNewer.IsEnabled = page > 1; CatalogOlder.IsEnabled = result.ReleasesMore;
            await LoadItemsAsync(1);
        }
        catch (OperationCanceledException) { CatalogStatus.Text = "Loading cancelled."; }
        catch (Exception error) { Problem(error.Message); }
        finally { _loading = false; CatalogRefresh.IsEnabled = true; }
    }
    private async Task LoadItemsAsync(uint page)
    {
        if (_auth is null || _browse.Active || _disposed || _loading) return;
        var artifact = Kind.SelectedIndex == 1;
        Release.Visibility = artifact ? Visibility.Collapsed : Visibility.Visible;
        RefreshAuthentication();
        if (artifact && !_auth.Connected) { Files.ItemsSource = null; Problem("Workflow artifacts require a GitHub account or token. Open Settings > GitHub to connect."); return; }
        if (!artifact && Release.SelectedItem is not SourceVersion) return;
        var engine = Engine;
        RefreshItems.IsEnabled = Previous.IsEnabled = Next.IsEnabled = false;
        Files.ItemsSource = null;
        try
        {
            var result = await _browse.RunAsync(_auth.Prepare(new GitHubQuery { Engine = engine, Page = page, Action = artifact ? GitHubQuery.Types.Action.Artifacts : GitHubQuery.Types.Action.ReleaseAssets, Id = (Release.SelectedItem as SourceVersion)?.Id ?? 0 }));
            if (_disposed || engine != Engine || artifact != (Kind.SelectedIndex == 1)) return;
            _page = page; Files.ItemsSource = result.Items.Select(i => new DownloadRow(i)).ToList();
            Previous.IsEnabled = page > 1; Next.IsEnabled = result.More;
            Notice.Severity = InfoBarSeverity.Informational; Notice.Title = "GitHub downloads"; Notice.Message = (result.Message != "GitHub data loaded." ? result.Message + " " : "") + (result.Items.Count == 0 ? "No files are available on this page." : $"{result.Items.Count} files · page {page}. Expired workflow artifacts cannot be downloaded."); Notice.IsOpen = true;
        }
        catch (OperationCanceledException) { CatalogStatus.Text = "Loading cancelled."; }
        catch (Exception error) { Problem(error.Message); }
        finally { RefreshItems.IsEnabled = true; Files_Changed(this, null!); }
    }
    private async void Download_Click(object sender, RoutedEventArgs e)
    {
        if (_auth is null || Files.SelectedItem is not DownloadRow row || row.Item.Expired) return;
        var artifact = Kind.SelectedIndex == 1;
        Download.IsEnabled = false; CancelDownload.IsEnabled = true; Progress.Visibility = Visibility.Visible; OpenFolder.IsEnabled = false;
        try
        {
            var result = await _download.RunAsync(_auth.Prepare(new GitHubQuery { Engine = Engine, Action = artifact ? GitHubQuery.Types.Action.DownloadArtifact : GitHubQuery.Types.Action.DownloadRelease, Id = row.Item.Id, OutputFolder = Folder.Text.Trim() }), s =>
            {
                DownloadStatus.Text = s.Message + (s.Received > 0 ? $" · {s.Received / 1048576d:N1} MiB" + (s.Total > 0 ? $" / {s.Total / 1048576d:N1} MiB" : "") : "");
                Progress.IsIndeterminate = s.Total == 0; if (s.Total > 0) Progress.Value = Math.Clamp(100d * s.Received / s.Total, 0, 100);
            });
            _output = result.Output; OpenFolder.IsEnabled = File.Exists(_output); DownloadStatus.Text = result.Message + " " + _output;
        }
        catch (Exception error) { DownloadStatus.Text = error.Message; }
        finally { CancelDownload.IsEnabled = false; Progress.Visibility = Visibility.Collapsed; Files_Changed(this, null!); }
    }
    private async void Engine_Changed(object sender, SelectionChangedEventArgs e) { if (_auth is null || _verification) return; _browse.Cancel(); while (_browse.Active && !_disposed) await Task.Delay(100); await LoadCatalogAsync(1); }
    private async void Kind_Changed(object sender, SelectionChangedEventArgs e) { if (_auth is not null && !_verification) { _browse.Cancel(); while (_browse.Active && !_disposed) await Task.Delay(100); await LoadItemsAsync(1); } }
    private async void Release_Changed(object sender, SelectionChangedEventArgs e) { if (!_loading && _auth is not null && !_verification) { _browse.Cancel(); while (_browse.Active && !_disposed) await Task.Delay(100); await LoadItemsAsync(1); } }
    private void Files_Changed(object sender, SelectionChangedEventArgs e) { if (Download is not null) Download.IsEnabled = !_download.Active && Files.SelectedItem is DownloadRow { Item.Expired: false }; }
    private async void CatalogRefresh_Click(object sender, RoutedEventArgs e) => await LoadCatalogAsync(1);
    private async void CatalogNewer_Click(object sender, RoutedEventArgs e) => await LoadCatalogAsync(Math.Max(1, _catalogPage - 1));
    private async void CatalogOlder_Click(object sender, RoutedEventArgs e) => await LoadCatalogAsync(_catalogPage + 1);
    private async void RefreshItems_Click(object sender, RoutedEventArgs e) => await LoadItemsAsync(1);
    private async void Previous_Click(object sender, RoutedEventArgs e) => await LoadItemsAsync(Math.Max(1, _page - 1));
    private async void Next_Click(object sender, RoutedEventArgs e) => await LoadItemsAsync(_page + 1);
    private void CancelLoading_Click(object sender, RoutedEventArgs e) => _browse.Cancel();
    private void CancelDownload_Click(object sender, RoutedEventArgs e) => _download.Cancel();
    private void OpenFolder_Click(object sender, RoutedEventArgs e) { if (File.Exists(_output)) Process.Start(new ProcessStartInfo(Path.GetDirectoryName(_output)!) { UseShellExecute = true }); }
    private async void Browse_Click(object sender, RoutedEventArgs e)
    {
        try { var picker = new FolderPicker(); picker.FileTypeFilter.Add("*"); WinRT.Interop.InitializeWithWindow.Initialize(picker, _owner); if (await picker.PickSingleFolderAsync() is { } folder) Folder.Text = folder.Path; }
        catch (Exception error) { Problem(error.Message); }
    }
    public async ValueTask DisposeAsync() { _disposed = true; await _browse.DisposeAsync(); await _download.DisposeAsync(); }
    internal void VerifyControls()
    {
        if (!_verification) throw new InvalidOperationException("Verification requires isolated state.");
        Files.ItemsSource = new[] { new DownloadRow(new DownloadItem { Id = 1, Name = "stockfish-windows-x86-64-bmi2.zip", Size = 10 * 1048576, Details = "Release asset" }), new DownloadRow(new DownloadItem { Id = 2, Name = "expired-workflow", Expired = true }) };
        Files.SelectedIndex = 1;
        if (Download.IsEnabled) throw new InvalidOperationException("An expired artifact can be downloaded.");
        Files.SelectedIndex = 0;
        if (!Download.IsEnabled || _download.Active) throw new InvalidOperationException("Download selection or independent session state failed.");
    }
    public sealed record DownloadRow(DownloadItem Item)
    {
        public string Name => Item.Name;
        public string SizeLabel => $"{Item.Size / 1048576d:N2} MiB · {Item.Date}";
        public string Details => (Item.Expired ? "Expired · " : "") + Item.Details;
    }
}
