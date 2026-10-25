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
        static void Require(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
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
        Engine.SelectedIndex = 1;
        Require(_engine == "lc0" && Lc0Options.Visibility == Visibility.Visible && StockfishOptions.Visibility == Visibility.Collapsed && MsysOptions.Visibility == Visibility.Collapsed, "Lc0 did not select its own controls.");
        Require(Model.SourceRef == "v0.32.1" && CompileButton.Content.ToString() == "Compile Lc0", "Lc0 source or action is wrong.");
        Require(Model.OutputRoot == stockfish.OutputRoot && Jobs.Value == 2, "First Lc0 selection lost shared output/jobs defaults.");
        Model.SourceRef = "release/0.32"; WeightsPath.Text = @"D:\Network folder\net.pb.gz";
        Backend.SelectedIndex = 1;
        Require(CudaRoot.Visibility == Visibility.Collapsed, "CPU selection still exposes CUDA toolkit settings.");
        var lc0 = Options();
        Engine.SelectedIndex = 0;
        Require(_engine == "stockfish" && StockfishOptions.Visibility == Visibility.Visible && Lc0Options.Visibility == Visibility.Collapsed, "Stockfish controls were not restored.");
        Require(Options().Equals(stockfish), "Switching engines changed Stockfish selections.");
        Engine.SelectedIndex = 1;
        Require(Options().Equals(lc0), "Switching engines lost Lc0 selections.");
        Backend.SelectedIndex = 0;
        Require(CudaRoot.Visibility == Visibility.Visible, "CUDA selection did not show its toolkit setting.");
        lc0 = Options();
        Engine.SelectedIndex = 2;
        Require(_engine == "berserk" && StockfishOptions.Visibility == Visibility.Visible && MsysOptions.Visibility == Visibility.Visible && Lc0Options.Visibility == Visibility.Collapsed && Jobs.Visibility == Visibility.Collapsed, "Berserk did not select its own controls.");
        Require(Model.SourceRef == "14" && CompileButton.Content.ToString() == "Compile Berserk", "Berserk source or action is wrong.");
        Require(Architecture.Items.Cast<Microsoft.UI.Xaml.Controls.ComboBoxItem>().Select(i => (string)i.Content).SequenceEqual(_hardware.BerserkChoices), "Berserk did not offer its own CPU targets.");
        Require(Options().Architecture == _hardware.BerserkRecommended && !Architecture.IsEnabled && Pgo.IsChecked == false, "Berserk did not default to its automatic recommendation without PGO.");
        AutoArchitecture.IsChecked = false; Architecture.SelectedIndex = 1; Pgo.IsChecked = true;
        var berserk = Options();
        Require(berserk.Architecture == "x86-64", "Manual Berserk target was not selected.");
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "berserk.png"));
        Engine.SelectedIndex = 0;
        Require(Options().Equals(stockfish) && Jobs.Visibility == Visibility.Visible, "Switching from Berserk changed Stockfish selections.");
        Engine.SelectedIndex = 2;
        Require(Options().Equals(berserk), "Switching engines lost Berserk selections.");
        Engine.SelectedIndex = 1;
        Require(Options().Equals(lc0), "Switching from Berserk changed Lc0 selections.");
        Engine.SelectedIndex = 3;
        Require(_engine == "obsidian" && Jobs.Visibility == Visibility.Visible && MsysOptions.Visibility == Visibility.Visible, "Obsidian controls are wrong.");
        Require(Model.SourceRef == "v16.0" && Options().Architecture == _hardware.ObsidianRecommended && !Pgo.IsChecked!.Value, "Obsidian defaults are wrong.");
        Require(Architecture.Items.Cast<Microsoft.UI.Xaml.Controls.ComboBoxItem>().Select(i => (string)i.Content).SequenceEqual(_hardware.ObsidianChoices), "Obsidian CPU choices are wrong.");
        AutoArchitecture.IsChecked = false; Architecture.SelectedIndex = 1; Strip.IsChecked = false;
        var obsidian = Options();
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "obsidian.png"));
        Engine.SelectedIndex = 0; Require(Options().Equals(stockfish), "Obsidian altered Stockfish settings.");
        Engine.SelectedIndex = 3; Require(Options().Equals(obsidian), "Obsidian settings were not retained.");
        Engine.SelectedIndex = 1; Require(Options().Equals(lc0), "Obsidian altered Lc0 settings.");
        Engine.SelectedIndex = 4;
        Require(_engine == "reckless" && MsysOptions.Visibility == Visibility.Collapsed && Pgo.Visibility == Visibility.Collapsed && RustRequirements.Visibility == Visibility.Visible, "Reckless exposed unsupported controls.");
        Require(Model.SourceRef == "v0.9.0" && Options().Architecture == _hardware.RecklessRecommended && !Options().Pgo, "Reckless defaults are wrong.");
        AutoArchitecture.IsChecked = false; Architecture.SelectedIndex = 1;
        var reckless = Options();
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "reckless.png"));
        Engine.SelectedIndex = 3; Require(Options().Equals(obsidian) && Pgo.Visibility == Visibility.Visible, "Reckless changed Obsidian settings.");
        Engine.SelectedIndex = 4; Require(Options().Equals(reckless), "Reckless settings were not retained.");
        Engine.SelectedIndex = 1;
        Engine.SelectedIndex = 5;
        Require(_engine == "igel" && MsysOptions.Visibility == Visibility.Visible && Pgo.Visibility == Visibility.Collapsed && Jobs.Visibility == Visibility.Collapsed, "Igel exposed unsupported controls.");
        Require(Model.SourceRef == "3.8.0" && Options().Architecture == _hardware.IgelRecommended && !Options().Pgo, "Igel defaults are wrong.");
        var igel = Options();
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "igel.png"));
        var realHardware = _hardware.Clone(); _hardware.IgelChoices.Clear(); _hardware.IgelRecommended = "";
        ShowArchitectures(); Require(Options().Architecture == "", "Unavailable Igel architecture did not remain empty.");
        _hardware = realHardware; LoadOptions(igel);
        Engine.SelectedIndex = 4; Require(Options().Equals(reckless), "Igel changed Reckless settings.");
        Engine.SelectedIndex = 5; Require(Options().Equals(igel), "Igel settings were not retained.");
        Engine.SelectedIndex = 1;
        Model.SourceRef = "v0.32.1";
        _logPresenter.ShowPreview("UI verification example\n\n$ \"git.exe\" fetch \"v0.32.1\"\n$ \"python.exe\" -m mesonbuild.mesonmain compile\nwarning: example diagnostic\nerror: example diagnostic\n\nNo build is running in this verification window.");
        Root.UpdateLayout(); await Task.Delay(200);
        Require(SourceRef.Text == Model.SourceRef, "Source revision binding was not connected to the control.");
        Require(Root.ActualWidth > 500 && Root.ActualHeight > 300, "Main window did not lay out.");
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "lc0.png"));
        OptionsScroll.ChangeView(null, OptionsScroll.ScrollableHeight, null, true);
        Root.UpdateLayout(); await Task.Delay(150);
        await CaptureUiAsync(Path.Combine(Path.GetDirectoryName(report)!, "lc0-tools.png"));
        Engine.SelectedIndex = 0;
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
        File.WriteAllText(report, "PASS: real WinUI engine switching across Stockfish, Lc0, Berserk, Obsidian, Reckless and Igel, separate option retention, per-engine automatic/manual architecture targets, official/development/custom source selection, SHA pinning, CUDA visibility, highlighted log, independent downloads/expiry controls, navigation to the settings page with the shared accent control, and rendered layouts. No build or shared preference mutation.");
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
        await DownloadPane.DisposeAsync();
        await Task.Run(_core.Dispose);
        _allowClose = true; Close();
    }
}
