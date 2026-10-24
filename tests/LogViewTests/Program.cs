using Jupiter.ChessEngineCompiler.App.Services;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace LogViewTests;

internal static class Program
{
    internal static string ResultPath = "";
    [STAThread]
    public static int Main(string[] args)
    {
        ResultPath = Path.GetFullPath(args[0]);
        WinRT.ComWrappersSupport.InitializeComWrappers();
        Application.Start(callback =>
        {
            SynchronizationContext.SetSynchronizationContext(new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread()));
            _ = new TestApp();
        });
        return File.ReadAllText(ResultPath).StartsWith("PASS") ? 0 : 1;
    }
}

public sealed partial class TestApp : Application
{
    private Window? _window;
    public TestApp() { InitializeComponent(); }
    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            var text = new TextBlock { IsTextSelectionEnabled = true, FontSize = 14 };
            var viewport = new ScrollViewer { Content = text, HorizontalScrollBarVisibility = ScrollBarVisibility.Auto };
            var follow = new CheckBox { IsChecked = true };
            var status = new TextBlock();
            var grid = new Grid(); grid.Children.Add(viewport);
            _window = new Window { Content = grid };
            _window.AppWindow.Resize(new Windows.Graphics.SizeInt32(1000, 700));
            // Render offscreen without taking focus from the user's application.
            _window.AppWindow.Move(new Windows.Graphics.PointInt32(-12000, -12000));
            _window.AppWindow.Show(false);
            var presenter = new LogPresenter(viewport, text, follow, status);
            string Lines(int start) => string.Join('\n', Enumerable.Range(start, 300).Select(n => $"Build output line {n:D6}"));
            async Task Settle(Func<bool>? ready = null)
            {
                for (var attempt = 0; attempt < 60; attempt++)
                {
                    grid.UpdateLayout(); await Task.Delay(50);
                    if (ready?.Invoke() ?? true) return;
                }
            }
            void Require(bool condition, string message) { if (!condition) throw new Exception(message); }
            bool AtBottom() => Math.Abs(viewport.VerticalOffset - viewport.ScrollableHeight) < 2;

            presenter.BeginBuild(); presenter.Update(Lines(0)); await Settle(() => viewport.ScrollableHeight > 100 && AtBottom());
            Require(viewport.ScrollableHeight > 100, "The real WinUI viewport was not laid out.");
            Require(AtBottom(), "Initial output did not scroll to the bottom.");
            presenter.Update(Lines(500)); await Settle();
            Require(AtBottom() && text.Text.EndsWith("000799"), "A rolling tail with unchanged height lost the latest lines.");

            presenter.Update(Lines(600)); // queues an automatic scroll
            presenter.Pause(); // simulates input arriving before that queued callback runs
            viewport.ChangeView(null, 50, null, true); await Settle(() => Math.Abs(viewport.VerticalOffset - 50) < 2);
            var frozen = text.Text; var offset = viewport.VerticalOffset;
            text.Select(text.ContentStart, text.ContentEnd);
            var selection = text.SelectedText;
            for (var i = 0; i < 10; i++) { presenter.Update(Lines(1000 + i * 300)); await Settle(); }
            Require(text.Text == frozen, "Updates replaced text while inspection was paused.");
            Require(Math.Abs(viewport.VerticalOffset - offset) < 2, "Updates moved the paused viewport.");
            Require(text.SelectedText == selection && selection.Length > 0, "Updates destroyed the text selection.");

            follow.IsChecked = true; await Settle(AtBottom);
            Require(AtBottom() && text.Text.EndsWith("003999"), "Resuming did not show the newest pending snapshot.");
            presenter.ShowPreview("Build preview"); await Settle();
            Require(viewport.VerticalOffset < 2 && text.Text == "Build preview", "Preview did not return to the top.");
            presenter.BeginBuild(); presenter.Update(Lines(5000)); await Settle(AtBottom);
            Require(AtBottom() && text.Text.EndsWith("005299"), "A new build did not resume following.");
            File.WriteAllText(Program.ResultPath, "PASS: real WinUI layout, bottom follow, rolling tail, paused scroll and selection, queued callback race, resume, preview and new build.");
        }
        catch (Exception error) { File.WriteAllText(Program.ResultPath, "FAIL: " + error); }
        finally { _window?.Close(); Exit(); }
    }
}
