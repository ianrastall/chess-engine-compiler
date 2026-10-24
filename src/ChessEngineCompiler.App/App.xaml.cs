using Microsoft.UI.Xaml;

namespace Jupiter.ChessEngineCompiler.App;

public partial class App : Application
{
    private MainWindow? _window;
    public App() { InitializeComponent(); }
    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        if (Program.UiTestReport is { } report)
        {
            Directory.CreateDirectory(Path.GetDirectoryName(report)!);
            try
            {
                _window = new MainWindow(Path.Combine(Path.GetDirectoryName(report)!, "state"));
                var scale = WindowDpi.GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(_window)) / 96.0;
                _window.AppWindow.Resize(new Windows.Graphics.SizeInt32((int)(1200 * scale), (int)(900 * scale)));
                _window.AppWindow.Move(new Windows.Graphics.PointInt32(-12000, -12000));
                _window.AppWindow.Show(false);
                await _window.VerifyEngineUiAsync(report);
            }
            catch (Exception error) { File.WriteAllText(report, "FAIL: " + error); }
            finally { if (_window is not null) await _window.CloseUiTestAsync(); Exit(); }
            return;
        }
        _window = new MainWindow();
        _window.Activate();
        _window.ConfigureWindow();
    }
}
