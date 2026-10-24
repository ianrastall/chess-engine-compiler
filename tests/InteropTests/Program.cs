using Jupiter.ChessEngineCompiler.Interop;
using Jupiter.ChessEngineCompiler.Messages;

using var core = new CompilerCore();
var defaults = core.Call(Request.Types.Operation.Defaults).Options;
if (defaults.Engine != "stockfish" || defaults.SourceRef != "sf_19") throw new Exception("Wrong defaults.");
for (var i = 0; i < 500; i++)
{
    var state = core.Call(Request.Types.Operation.Status).Snapshot;
    if (state.State != "Ready") throw new Exception("Unexpected initial status.");
}
defaults.SourceRef = "master;calc";
try { core.Call(Request.Types.Operation.Start, defaults); throw new Exception("Invalid source ref was accepted."); }
catch (InvalidOperationException e) when (e.Message.Contains("Stockfish tag")) { }
core.Call(Request.Types.Operation.Cancel);
Console.WriteLine("Managed/native round-trip, result lifetime, validation and idle cancellation passed.");
if (args is ["--export-smoke", var app, var parent, var config])
{
    var options = Google.Protobuf.JsonParser.Default.Parse<BuildOptions>(File.ReadAllText(config));
    Console.WriteLine(Jupiter.ChessEngineCompiler.App.Services.ScriptExporter.Export(options, parent, app));
}
