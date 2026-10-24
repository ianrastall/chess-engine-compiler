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
        Model.SourceRef = "v0.32.1";
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
        File.WriteAllText(report, "PASS: real WinUI engine switching, separate option retention, shared defaults, engine-specific controls/actions, CUDA visibility and rendered layouts. No build or shared preference mutation.");
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
        await Task.Run(_core.Dispose);
        _allowClose = true; Close();
    }
}
