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
catch (InvalidOperationException e) when (e.Message.Contains("source tag")) { }
core.Call(Request.Types.Operation.Cancel);
var lc0 = core.Call(Request.Types.Operation.Defaults, new BuildOptions { Engine = "lc0" }).Options;
if (lc0.Engine != "lc0" || lc0.SourceRef != "v0.32.1" || lc0.Pgo || lc0.Strip) throw new Exception("Wrong Lc0 defaults.");
lc0.Backend = "unsupported"; lc0.OutputRoot = Path.GetTempPath();
try { core.Call(Request.Types.Operation.Plan, lc0); throw new Exception("Unknown Lc0 backend was accepted."); }
catch (InvalidOperationException e) when (e.Message.Contains("Eigen")) { }
lc0.PythonPath = @"C:\tools\python.exe"; lc0.WeightsPath = @"D:\networks\network.pb.gz";
var roundTrip = Google.Protobuf.JsonParser.Default.Parse<BuildOptions>(Jupiter.ChessEngineCompiler.App.Services.ScriptExporter.SerializeOptions(lc0));
if (!roundTrip.Equals(lc0)) throw new Exception("Lc0 script configuration did not round-trip.");
Console.WriteLine("Managed/native round-trip, result lifetime, validation and idle cancellation passed.");
if (args is ["--export-smoke", var app, var parent, var config])
{
    var options = Google.Protobuf.JsonParser.Default.Parse<BuildOptions>(File.ReadAllText(config));
    Console.WriteLine(Jupiter.ChessEngineCompiler.App.Services.ScriptExporter.Export(options, parent, app));
}
