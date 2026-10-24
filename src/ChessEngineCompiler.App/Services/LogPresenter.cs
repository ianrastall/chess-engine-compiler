using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using VirtualKey = Windows.System.VirtualKey;

namespace Jupiter.ChessEngineCompiler.App.Services;

/// <summary>The log owns its scroll viewport; inspecting text freezes the displayed snapshot.</summary>
public sealed class LogPresenter
{
    private readonly ScrollViewer _viewport;
    private readonly TextBlock _text;
    private readonly CheckBox _follow;
    private readonly TextBlock _status;
    private string _latest = "";
    private bool _pendingScroll;

    public LogPresenter(ScrollViewer viewport, TextBlock text, CheckBox follow, TextBlock status)
    {
        (_viewport, _text, _follow, _status) = (viewport, text, follow, status);
        follow.Checked += (_, _) => Resume();
        follow.Unchecked += (_, _) =>
        {
            _pendingScroll = false;
            status.Text = "Log view paused. Select Follow output to show the latest lines.";
        };
        text.SizeChanged += (_, _) => ScrollToEnd();
        viewport.SizeChanged += (_, _) => ScrollToEnd();
        viewport.LayoutUpdated += (_, _) => ApplyPendingScroll();
        viewport.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler((_, _) => Pause()), true);
        viewport.AddHandler(UIElement.PointerWheelChangedEvent, new PointerEventHandler((_, e) =>
        {
            if (e.GetCurrentPoint(viewport).Properties.MouseWheelDelta > 0) Pause();
        }), true);
        viewport.AddHandler(UIElement.KeyDownEvent, new KeyEventHandler((_, e) =>
        {
            if (e.Key is VirtualKey.Up or VirtualKey.PageUp or VirtualKey.Home or VirtualKey.Left) Pause();
        }), true);
    }

    public void Pause() => _follow.IsChecked = false;

    public void Update(string text)
    {
        _latest = text;
        if (_follow.IsChecked == true && _text.Text != text)
        {
            _text.Text = text;
            ScrollToEnd();
        }
    }

    public void BeginBuild()
    {
        _latest = "";
        _follow.IsChecked = true;
        Resume();
    }

    public void ShowPreview(string text)
    {
        Pause();
        _latest = _text.Text = text;
        _status.Text = "Build preview; compilation has not started.";
        _viewport.ChangeView(0, 0, null, true);
    }

    private void Resume()
    {
        _status.Text = "Latest 300 lines; the full build log is saved to disk.";
        Update(_latest);
        ScrollToEnd();
    }

    private void ScrollToEnd()
    {
        if (_follow.IsChecked != true) return;
        _pendingScroll = true;
        _viewport.DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, ApplyPendingScroll);
    }

    private void ApplyPendingScroll()
    {
        // Text size can update before the ScrollViewer has incorporated its new extent.
        // Leave the request pending until layout catches up, including on first render.
        if (!_pendingScroll || _follow.IsChecked != true || _viewport.ViewportHeight <= 0
            || _viewport.ExtentHeight < _text.ActualHeight - 1) return;
        _pendingScroll = false;
        _viewport.ChangeView(null, _viewport.ScrollableHeight, null, true);
    }
}
