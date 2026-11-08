using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Graphics.Imaging;
using Windows.Storage;
using Windows.Storage.Streams;

namespace Jupiter.ChessEngineCompiler.App;

public sealed partial class MainWindow
{
    // Runs through the real controls without input injection or shared user state.
    internal async Task VerifyEngineUiAsync(string report)
    {
        Bindings.Update();
        void SelectEngine(string id) => Engine.SelectedIndex = EngineCatalog.Index(id);
        if (!EngineCatalog.Items.Select(e => e.Name).SequenceEqual(EngineCatalog.Items.Select(e => e.Name).OrderBy(n => n, StringComparer.OrdinalIgnoreCase))) throw new InvalidOperationException("Engine dropdown is not alphabetical.");
        static void Require(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
        Require(EngineCatalog.Items.Select(e => e.Id).SequenceEqual(new[] { "lc0", "stockfish" }), "The 1.0 catalog must contain Stockfish and Lc0.");
        Require(LibraryPane.Visibility == Visibility.Visible && OptionsScroll.Visibility == Visibility.Collapsed, "The release library is not the startup view.");
        Directory.CreateDirectory(CatalogRoot);
        void Fixture(string engine, string[] tags)
        {
            var releases = tags.Select((tag, i) => new { id = i + 1, label = tag, reference = tag, date = $"2026-01-0{9 - i}T00:00:00Z", url = "", prerelease = false });
            File.WriteAllText(Path.Combine(CatalogRoot, $"{engine}-releases.json"),
                System.Text.Json.JsonSerializer.Serialize(new { version = 1, baseline = new[] { 3 }, releases }));
        }
        Fixture("stockfish", ["sf_19", "sf_18", "sf_12"]);
        Fixture("lc0", ["v0.32.1", "v0.32.0", "v0.31.2"]);
        PreviewLibrary();
        Require(_libraryRows.Select(row => row.Tag).SequenceEqual(new[] { "sf_19", "v0.32.1", "sf_18", "v0.32.0" }), "The release library did not order the two initial releases newest first across engines.");
        LibraryScope.SelectedIndex = 2;
        Require(_libraryRows.Count(row => row.State == "Needs recipe update") == 2, "Unsupported historical recipes were not labeled.");
        LibraryEngine.SelectedIndex = 2;
        Require(_libraryRows.All(row => row.Engine == "lc0") && _libraryRows.Length == 3, "The library engine filter did not apply.");
        LibraryEngine.SelectedIndex = 0;
        LibraryScope.SelectedIndex = 0;
        Root.UpdateLayout(); await Task.Delay(200);
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "release-library.png"));
        ShowArea("compile");
        AutoArchitecture.IsChecked = true;
        Require(!Architecture.IsEnabled && Options().Architecture == _hardware.Recommended, "Automatic architecture did not select a compatible recommendation.");
        AutoArchitecture.IsChecked = false;
        _loadingSources = true;
        OfficialVersions.ItemsSource = new[] { new Messages.SourceVersion { Label = "Stockfish 19", Reference = "sf_19" } };
        DevelopmentVersions.ItemsSource = new[] { new Messages.SourceVersion { Label = "Development fixture", Reference = new string('a', 40) } };
        _loadingSources = false;
        SourceChannel.SelectedIndex = 0;
        Require(Model.SourceRef == "sf_19" && SourceRef.IsReadOnly, "Official source selection did not use its tag.");
        SourceChannel.SelectedIndex = 1;
        Require(Model.SourceRef == new string('a', 40), "Development selection did not pin its commit SHA.");
        SourceChannel.SelectedIndex = 2;
        Require(!SourceRef.IsReadOnly, "Custom refs are not editable.");
        Model.SourceRef = "sf_19"; Model.OutputRoot = @"D:\Example output";
        Jobs.Value = 2; Pgo.IsChecked = true; Architecture.SelectedIndex = 3;
        var stockfish = Options();
        SelectEngine("lc0");
        Require(_engine == "lc0" && Lc0Options.Visibility == Visibility.Visible && CpuBuildOptions.Visibility == Visibility.Collapsed && MsysOptions.Visibility == Visibility.Collapsed, "Lc0 did not select its own controls.");
        Require(Model.SourceRef == "v0.32.1" && CompileButton.Content.ToString() == "Compile Lc0", "Lc0 source or action is wrong.");
        Require(Model.OutputRoot == stockfish.OutputRoot && Jobs.Value == 2, "First Lc0 selection lost shared output/jobs defaults.");
        Model.SourceRef = "release/0.32"; WeightsPath.Text = @"D:\Network folder\net.pb.gz";
        Backend.SelectedIndex = 1;
        Require(CudaRoot.Visibility == Visibility.Collapsed, "CPU selection still exposes CUDA toolkit settings.");
        var lc0 = Options();
        SelectEngine("stockfish");
        Require(_engine == "stockfish" && CpuBuildOptions.Visibility == Visibility.Visible && Lc0Options.Visibility == Visibility.Collapsed, "Stockfish controls were not restored.");
        Require(Options().Equals(stockfish), "Switching engines changed Stockfish selections.");
        SelectEngine("lc0");
        Require(Options().Equals(lc0), "Switching engines lost Lc0 selections.");
        Backend.SelectedIndex = 0;
        Require(CudaRoot.Visibility == Visibility.Visible, "CUDA selection did not show its toolkit setting.");
        lc0 = Options();
        SelectEngine("lc0");
        Model.SourceRef = "v0.32.1";
        _logPresenter.ShowPreview("UI verification example\n\n$ \"git.exe\" fetch \"v0.32.1\"\n$ \"python.exe\" -m mesonbuild.mesonmain compile\nwarning: example diagnostic\nerror: example diagnostic\n\nNo build is running in this verification window.");
        Root.UpdateLayout(); await Task.Delay(200);
        Require(SourceRef.Text == Model.SourceRef, "Source revision binding was not connected to the control.");
        Require(Root.ActualWidth > 500 && Root.ActualHeight > 300, "Main window did not lay out.");
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "lc0.png"));
        OptionsScroll.ChangeView(null, OptionsScroll.ScrollableHeight, null, true);
        Root.UpdateLayout(); await Task.Delay(150);
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "lc0-tools.png"));
        SelectEngine("stockfish");
        ShowArea("library");
        LibraryResults.SelectedIndex = 0;
        Require(LibraryWeightsButton.Visibility == Visibility.Collapsed, "Stockfish exposes the Lc0 weights action.");
        LibraryResults.SelectedIndex = 1;
        Require(LibraryWeightsButton.Visibility == Visibility.Visible && LibraryWeightsButton.IsEnabled, "Lc0 weights are inaccessible from the release row.");
        ManageWeights_Click(LibraryWeightsButton, null!);
        Require(_engine == "lc0" && OptionsScroll.Visibility == Visibility.Visible && DownloadWeightsButton.IsEnabled,
            "The release row did not open the Lc0 weights controls.");
        Require(Options().WeightsPath == lc0.WeightsPath, "Opening weights controls changed the custom weights selection.");
        SelectEngine("stockfish");
        OptionsScroll.ChangeView(null, 0, null, true);
        Root.UpdateLayout(); await Task.Delay(150);
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "stockfish.png"));
        ShowArea("downloads");
        DownloadPane.VerifyControls();
        Root.UpdateLayout(); await Task.Delay(200);
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "downloads.png"));
        ShowArea("settings");
        Require(SettingsPage.Visibility == Visibility.Visible && AccentSetting.Service is not null, "Settings page or its shared accent control is not connected.");
        Root.UpdateLayout(); await Task.Delay(200);
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "settings.png"));
        await ToolSetupPane.VerifyControlsAsync();
        ToolSetupPane.StartBringIntoView(); Root.UpdateLayout(); await Task.Delay(200);
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "build-tools.png"));
        File.WriteAllText(report, "PASS: real WinUI alphabetical engine selection, the startup release library, initial ordering, scope and engine filters, unsupported recipe status, switching across both engines, separate option retention, per-engine automatic/manual architecture targets, official/development/custom source selection, SHA pinning, CUDA visibility, highlighted log, independent downloads/expiry controls, navigation to the settings page with the shared accent control, and rendered layouts. No build or shared preference mutation.");
    }

    private async Task CaptureUiAsync(string path)
    {
        var bitmap = new RenderTargetBitmap();
        await bitmap.RenderAsync(Root);
        using var reader = DataReader.FromBuffer(await bitmap.GetPixelsAsync());
        var pixels = new byte[reader.UnconsumedBufferLength]; reader.ReadBytes(pixels);
        var folder = await StorageFolder.GetFolderFromPathAsync(Path.GetDirectoryName(Path.GetFullPath(path))!);
        var file = await folder.CreateFileAsync(Path.GetFileName(path), CreationCollisionOption.ReplaceExisting);
        using var stream = await file.OpenAsync(FileAccessMode.ReadWrite);
        var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, stream);
        encoder.SetPixelData(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied, (uint)bitmap.PixelWidth, (uint)bitmap.PixelHeight, 96, 96, pixels);
        await encoder.FlushAsync();
    }

    internal async Task CloseUiTestAsync()
    {
        _timer.Stop();
        await _sourceSession.DisposeAsync();
        await _librarySession.DisposeAsync();
        await DownloadPane.DisposeAsync();
        await ToolSetupPane.DisposeAsync();
        await Task.Run(_core.Dispose);
        _allowClose = true; Close();
    }
}
