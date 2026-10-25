using Jupiter.ChessEngineCompiler.App.Services;
using Jupiter.ChessEngineCompiler.Messages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;

namespace Jupiter.ChessEngineCompiler.App;

public sealed partial class MainWindow
{
    private int _sourceGeneration;
    private uint _sourcePage = 1;
    private bool _loadingSources;
    private ContentDialog? _githubDialog;
    private GitHubSession? _authSession;
    private Task? _githubDialogTask;
    private async Task LoadSourcesAsync(uint page)
    {
        var generation = ++_sourceGeneration;
        var engine = _engine;
        try
        {
            _sourceSession.Cancel();
            while (_sourceSession.Active && !_closing) await Task.Delay(100);
            if (_closing || generation != _sourceGeneration) return;
            RefreshSources.IsEnabled = NewerSources.IsEnabled = OlderSources.IsEnabled = false;
            CancelSources.Visibility = Visibility.Visible;
            SourceStatus.Text = "Loading releases and development commits from GitHub…";
            var snapshot = await _sourceSession.RunAsync(_githubCredentials.Prepare(new GitHubQuery { Engine = engine, Page = page, Action = GitHubQuery.Types.Action.Catalog }));
            if (_closing || generation != _sourceGeneration || engine != _engine) return;
            if (_active) { SourceStatus.Text = "Versions loaded while compilation was active. Refresh after the build to select a version."; return; }
            _sourcePage = page; _loadingSources = true;
            OfficialVersions.ItemsSource = snapshot.Releases.Where(v => !v.Prerelease).ToList();
            DevelopmentVersions.ItemsSource = snapshot.Development.ToList();
            OfficialVersions.SelectedItem = snapshot.Releases.FirstOrDefault(v => !v.Prerelease && v.Reference == Model.SourceRef);
            DevelopmentVersions.SelectedItem = snapshot.Development.FirstOrDefault(v => v.Reference == Model.SourceRef);
            if (OfficialVersions.SelectedItem is not null) SourceChannel.SelectedIndex = 0;
            else if (DevelopmentVersions.SelectedItem is not null) SourceChannel.SelectedIndex = 1;
            _loadingSources = false;
            SourceChannel_Changed(this, null!);
            SourceStatus.Text = $"GitHub page {page} · development branch: {snapshot.DefaultBranch}. Development commits use exact SHAs.";
            OlderSources.IsEnabled = snapshot.More; NewerSources.IsEnabled = page > 1;
        }
        catch (OperationCanceledException) { if (generation == _sourceGeneration) SourceStatus.Text = "Version loading cancelled. Custom refs remain available."; }
        catch (Exception error) { if (generation == _sourceGeneration) SourceStatus.Text = error.Message + " You can still enter a custom tag, branch or commit."; }
        finally { if (generation == _sourceGeneration) { _loadingSources = false; RefreshSources.IsEnabled = true; CancelSources.Visibility = Visibility.Collapsed; } }
    }
    private void SourceChannel_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (_loadingSources || OfficialVersions is null || DevelopmentVersions is null || SourceRef is null) return;
        OfficialVersions.IsEnabled = SourceChannel.SelectedIndex == 0;
        DevelopmentVersions.IsEnabled = SourceChannel.SelectedIndex == 1;
        SourceRef.IsReadOnly = SourceChannel.SelectedIndex != 2;
        if (SourceChannel.SelectedIndex == 0)
        {
            if (OfficialVersions.SelectedItem is null && OfficialVersions.Items.Count > 0) OfficialVersions.SelectedIndex = 0;
            if (OfficialVersions.SelectedItem is SourceVersion version) Model.SourceRef = version.Reference;
        }
        if (SourceChannel.SelectedIndex == 1)
        {
            if (DevelopmentVersions.SelectedItem is null && DevelopmentVersions.Items.Count > 0) DevelopmentVersions.SelectedIndex = 0;
            if (DevelopmentVersions.SelectedItem is SourceVersion version) Model.SourceRef = version.Reference;
        }
    }
    private void OfficialVersion_Changed(object sender, SelectionChangedEventArgs e) { if (!_loadingSources && SourceChannel.SelectedIndex == 0 && OfficialVersions.SelectedItem is SourceVersion version) Model.SourceRef = version.Reference; }
    private void DevelopmentVersion_Changed(object sender, SelectionChangedEventArgs e) { if (!_loadingSources && SourceChannel.SelectedIndex == 1 && DevelopmentVersions.SelectedItem is SourceVersion version) Model.SourceRef = version.Reference; }
    private async void RefreshSources_Click(object sender, RoutedEventArgs e) => await LoadSourcesAsync(1);
    private async void NewerSources_Click(object sender, RoutedEventArgs e) => await LoadSourcesAsync(Math.Max(1, _sourcePage - 1));
    private async void OlderSources_Click(object sender, RoutedEventArgs e) => await LoadSourcesAsync(_sourcePage + 1);
    private void CancelSources_Click(object sender, RoutedEventArgs e) => _sourceSession.Cancel();
    private void AutoArchitecture_Changed(object sender, RoutedEventArgs e)
    {
        if (_loadingOptions || Architecture is null || AutoArchitecture is null) return;
        Architecture.IsEnabled = AutoArchitecture.IsChecked != true;
        if (AutoArchitecture.IsChecked == true) foreach (ComboBoxItem item in Architecture.Items) if ((string)item.Content == _hardware.Recommended) Architecture.SelectedItem = item;
    }
    private async void CopyLog_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            var text = _logPresenter.LatestText;
            if (!_logPresenter.IsPreview && File.Exists(_logPath))
            {
                await using var stream = new FileStream(_logPath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
                using var reader = new StreamReader(stream); text = await reader.ReadToEndAsync();
            }
            var data = new DataPackage(); data.SetText(text); Clipboard.SetContent(data); Clipboard.Flush();
            Notice.Severity = InfoBarSeverity.Informational; Notice.Title = _logPresenter.IsPreview ? "Preview copied" : "Log copied"; Notice.Message = _logPresenter.IsPreview ? "The current preview was copied to the clipboard." : "The full saved log was copied to the clipboard."; Notice.IsOpen = true;
        }
        catch (Exception error) { ShowProblem(error.Message); }
    }
    private async void GitHubSettings_Click(object sender, RoutedEventArgs e)
    {
        await using var session = new GitHubSession();
        _authSession = session;
        var status = new TextBox { Text = $"Current access: {_githubCredentials.Mode}", IsReadOnly = true, TextWrapping = TextWrapping.Wrap, MinHeight = 64 };
        var password = new PasswordBox { Header = "GitHub personal access token", PlaceholderText = _githubCredentials.Mode == "token" ? "•••••••• · saved token; type a replacement to change it" : "Paste a personal access token", PasswordRevealMode = PasswordRevealMode.Hidden };
        ToolTipService.SetToolTip(password, "Stored in Windows Credential Manager. It is never written to build settings, scripts, logs or exported files. Fine-grained tokens need Actions read access for workflow artifacts in the selected upstream repositories.");
        var save = new Button { Content = "Validate and save token" };
        var existing = new Button { Content = "Use GitHub CLI account" };
        var signIn = new Button { Content = "Sign in through browser" };
        var disconnect = new Button { Content = "Disconnect this app" };
        var cancel = new Button { Content = "Cancel sign-in", IsEnabled = false };
        async Task ConnectAsync(GitHubQuery query, string mode, string? token = null)
        {
            save.IsEnabled = existing.IsEnabled = signIn.IsEnabled = disconnect.IsEnabled = false; cancel.IsEnabled = true;
            status.Text = "Testing GitHub connection…";
            try
            {
                var result = await session.RunAsync(query, s => status.Text = s.Message);
                if (token is not null) _githubCredentials.SaveToken(token); else _githubCredentials.SelectMode(mode);
                var method = mode == "cli" ? "GitHub CLI" : "a personal access token";
                password.Password = ""; status.Text = $"Connection test successful.\nConnected as {result.Login} using {method}. Your settings are saved; you can close this dialog.";
            }
            catch (OperationCanceledException) { status.Text = "Connection test cancelled."; }
            catch (Exception error) { status.Text = $"Connection test failed.\n{error.Message}"; }
            finally { save.IsEnabled = existing.IsEnabled = signIn.IsEnabled = disconnect.IsEnabled = true; cancel.IsEnabled = false; }
        }
        save.Click += async (_, _) => { var token = password.Password.Trim(); if (token.Length == 0) { status.Text = "Enter a personal access token."; return; } await ConnectAsync(new GitHubQuery { Engine = _engine, AuthMode = "token", Token = token, Action = GitHubQuery.Types.Action.Account }, "token", token); };
        existing.Click += async (_, _) => await ConnectAsync(new GitHubQuery { Engine = _engine, AuthMode = "cli", Action = GitHubQuery.Types.Action.Account }, "cli");
        signIn.Click += async (_, _) => await ConnectAsync(new GitHubQuery { Engine = _engine, AuthMode = "cli", Action = GitHubQuery.Types.Action.SignIn }, "cli");
        cancel.Click += (_, _) => session.Cancel();
        disconnect.Click += (_, _) => { try { _githubCredentials.Disconnect(); status.Text = "This app now uses anonymous access. GitHub CLI stays signed in for other applications."; } catch (Exception error) { status.Text = error.Message; } };
        var panel = new StackPanel { Spacing = 12, MaxWidth = 520 };
        panel.Children.Add(new TextBlock { Text = "Authenticated access raises API limits and enables workflow artifact downloads. Browser sign-in uses GitHub CLI’s device-code flow and opens GitHub for your authorization. GitHub CLI must be installed separately.", TextWrapping = TextWrapping.Wrap });
        panel.Children.Add(password); panel.Children.Add(save); panel.Children.Add(existing); panel.Children.Add(signIn); panel.Children.Add(cancel); panel.Children.Add(disconnect); panel.Children.Add(status);
        panel.Children.Add(new HyperlinkButton { Content = "GitHub CLI installation", NavigateUri = new Uri("https://cli.github.com/"), Padding = new Thickness(0) });
        var dialog = new ContentDialog { XamlRoot = Root.XamlRoot, Title = "GitHub", Content = new ScrollViewer { Content = panel }, CloseButtonText = "Close" };
        dialog.Closing += (_, _) => session.Cancel();
        _githubDialog = dialog;
        _githubDialogTask = dialog.ShowAsync().AsTask();
        try { await _githubDialogTask; }
        finally { _githubDialog = null; _githubDialogTask = null; _authSession = null; }
        DownloadPane.RefreshAuthentication();
        UpdateGitHubAccess();
        if (!_verification && !_closing) _ = LoadSourcesAsync(_sourcePage);
    }
    private void PowerShellLog_Click(object sender, RoutedEventArgs e)
    {
        if (!File.Exists(_logPath)) return;
        try
        {
            var host = Environment.GetEnvironmentVariable("PATH")?.Split(Path.PathSeparator).Select(p => Path.Combine(p, "pwsh.exe")).FirstOrDefault(File.Exists)
                ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), "PowerShell", "7", "pwsh.exe");
            if (!File.Exists(host)) host = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows), "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
            var start = new System.Diagnostics.ProcessStartInfo(host) { UseShellExecute = false };
            foreach (var argument in new[] { "-NoLogo", "-NoProfile", "-NoExit", "-Command", "Get-Content -LiteralPath $env:CEC_LOG_PATH -Wait | ForEach-Object { $line=$_; $color=if($line -match '^\\$ '){'Cyan'}elseif($line -match 'warning'){'Yellow'}elseif($line -match 'error|^FAILED:'){'Red'}else{'Gray'}; Write-Host $line -ForegroundColor $color }" }) start.ArgumentList.Add(argument);
            start.Environment["CEC_LOG_PATH"] = _logPath;
            System.Diagnostics.Process.Start(start);
        }
        catch (Exception error) { ShowProblem(error.Message); }
    }
}
