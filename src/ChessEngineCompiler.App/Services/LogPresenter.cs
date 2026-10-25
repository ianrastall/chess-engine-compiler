using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using VirtualKey = Windows.System.VirtualKey;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;
using System.Text.RegularExpressions;

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
    private bool _rendering;
    public string LatestText => _latest;
    public bool IsPreview { get; private set; }

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
        text.SelectionChanged += (_, _) => { if (!_rendering && text.SelectedText.Length > 0) Pause(); };
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
            Render(text);
            ScrollToEnd();
        }
    }

    public void BeginBuild()
    {
        IsPreview = false;
        _latest = "";
        _follow.IsChecked = true;
        Resume();
    }

    public void ShowPreview(string text)
    {
        IsPreview = true;
        Pause();
        _latest = text; Render(text);
        _status.Text = "Build preview; compilation has not started.";
        _viewport.ChangeView(0, 0, null, true);
    }

    private void Resume()
    {
        _status.Text = "Latest 300 lines; the full build log is saved to disk.";
        Update(_latest);
        ScrollToEnd();
    }

    private void Render(string value)
    {
        _rendering = true;
        try
        {
            _text.Select(_text.ContentStart, _text.ContentStart);
            // Runs retain ordinary text selection/copy. Color never alters the saved log.
            _text.Inlines.Clear();
            var contrast = new Windows.UI.ViewManagement.AccessibilitySettings().HighContrast;
            // Command and literal colors are syntax highlighting, a content palette; warnings and errors
            // are status and use the Jupiter status colors (style standard 3.1 and 3.4).
            var command = new SolidColorBrush(Windows.UI.Color.FromArgb(255, 124, 193, 229));
            var literal = new SolidColorBrush(Windows.UI.Color.FromArgb(255, 170, 199, 145));
            var warning = Jupiter.UI.JupiterTheme.Brush("JupiterWarningBrush");
            var error = Jupiter.UI.JupiterTheme.Brush("JupiterErrorBrush");
            foreach (var line in Regex.Split(value, "(?<=\\n)"))
            {
                if (!contrast && line.StartsWith("$ ", StringComparison.Ordinal))
                {
                    foreach (Match token in Regex.Matches(line, "\"(?:\\\\.|[^\"\\\\])*\"|'[^']*'|[^\"']+"))
                        _text.Inlines.Add(new Run { Text = token.Value, Foreground = token.Value.StartsWith('"') || token.Value.StartsWith('\'') ? literal : command });
                }
                else _text.Inlines.Add(new Run { Text = line, Foreground = contrast ? _text.Foreground : line.Contains("error", StringComparison.OrdinalIgnoreCase) || line.StartsWith("FAILED:", StringComparison.Ordinal) ? error : line.Contains("warning", StringComparison.OrdinalIgnoreCase) ? warning : _text.Foreground });
            }
            _text.Select(_text.ContentStart, _text.ContentStart);
        }
        finally { _rendering = false; }
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
