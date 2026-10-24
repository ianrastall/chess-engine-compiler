using Jupiter.ChessEngineCompiler.Interop;
using Jupiter.ChessEngineCompiler.Messages;

namespace Jupiter.ChessEngineCompiler.App.Services;

/// <summary>Short typed calls poll a Rust worker; HTTP and downloads never block the UI.</summary>
public sealed class GitHubSession : IAsyncDisposable
{
    private readonly CompilerCore _core = new();
    private bool _active, _disposed;
    public bool Active => _active;
    public async Task<GitHubSnapshot> RunAsync(GitHubQuery query, Action<GitHubSnapshot>? progress = null)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
        if (_active) throw new InvalidOperationException("A GitHub operation is already running. Cancel it or wait for it to finish.");
        _active = true;
        try
        {
            _core.Call(Request.Types.Operation.GithubStart, github: query);
            while (!_disposed)
            {
                var status = _core.Call(Request.Types.Operation.GithubStatus).Github;
                progress?.Invoke(status);
                if (status.State == "Completed") return status;
                if (status.State == "Failed") throw new InvalidOperationException(status.Message);
                if (status.State == "Cancelled") throw new OperationCanceledException(status.Message);
                await Task.Delay(150);
            }
            throw new OperationCanceledException();
        }
        finally { query.Token = ""; _active = false; }
    }
    public void Cancel() { if (!_disposed && _active) _core.Call(Request.Types.Operation.GithubCancel); }
    public async ValueTask DisposeAsync()
    {
        if (_disposed) return;
        Cancel(); _disposed = true;
        await Task.Run(_core.Dispose);
    }
}
