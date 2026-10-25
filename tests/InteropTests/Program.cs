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
var hardware = core.Call(Request.Types.Operation.Hardware).Hardware;
if (!hardware.Choices.Contains(hardware.Recommended)) throw new Exception("Unsupported architecture recommendation.");
if (!hardware.BerserkChoices.Contains(hardware.BerserkRecommended)) throw new Exception("Unsupported Berserk architecture recommendation.");
var berserk = core.Call(Request.Types.Operation.Defaults, new BuildOptions { Engine = "berserk" }).Options;
if (berserk.Engine != "berserk" || berserk.SourceRef != "14" || berserk.Pgo || !berserk.Strip || !berserk.AutoArchitecture || berserk.Architecture != hardware.BerserkRecommended) throw new Exception("Wrong Berserk defaults.");
berserk.OutputRoot = Path.GetTempPath(); berserk.AutoArchitecture = false; berserk.Architecture = "x86-64-avx2";
try { core.Call(Request.Types.Operation.Plan, berserk); throw new Exception("A Stockfish target was accepted for Berserk."); }
catch (InvalidOperationException e) when (e.Message.Contains("Berserk target")) { }
var berserkRoundTrip = Google.Protobuf.JsonParser.Default.Parse<BuildOptions>(Jupiter.ChessEngineCompiler.App.Services.ScriptExporter.SerializeOptions(berserk));
if (!berserkRoundTrip.Equals(berserk)) throw new Exception("Berserk script configuration did not round-trip.");
var obsidian = core.Call(Request.Types.Operation.Defaults, new BuildOptions { Engine = "obsidian" }).Options;
if (obsidian.SourceRef != "v16.0" || obsidian.Architecture != hardware.ObsidianRecommended || !hardware.ObsidianChoices.Contains(obsidian.Architecture)) throw new Exception("Wrong Obsidian defaults/hardware.");
obsidian.OutputRoot = Path.GetTempPath(); obsidian.AutoArchitecture = false; obsidian.Architecture = "x86-64";
try { core.Call(Request.Types.Operation.Plan, obsidian); throw new Exception("Stockfish target accepted for Obsidian."); }
catch (InvalidOperationException e) when (e.Message.Contains("Obsidian target")) { }
if (!Google.Protobuf.JsonParser.Default.Parse<BuildOptions>(Jupiter.ChessEngineCompiler.App.Services.ScriptExporter.SerializeOptions(obsidian)).Equals(obsidian)) throw new Exception("Obsidian script configuration did not round-trip.");
var reckless = core.Call(Request.Types.Operation.Defaults, new BuildOptions { Engine = "reckless" }).Options;
if (reckless.SourceRef != "v0.9.0" || reckless.Pgo || reckless.Architecture != hardware.RecklessRecommended || !hardware.RecklessChoices.Contains(reckless.Architecture)) throw new Exception("Wrong Reckless defaults/hardware.");
reckless.OutputRoot = Path.GetTempPath(); reckless.Pgo = true;
try { core.Call(Request.Types.Operation.Plan, reckless); throw new Exception("Unsupported Reckless PGO accepted."); }
catch (InvalidOperationException e) when (e.Message.Contains("PGO")) { }
if (!Google.Protobuf.JsonParser.Default.Parse<BuildOptions>(Jupiter.ChessEngineCompiler.App.Services.ScriptExporter.SerializeOptions(reckless)).Equals(reckless)) throw new Exception("Reckless script configuration did not round-trip.");
var igel = core.Call(Request.Types.Operation.Defaults, new BuildOptions { Engine = "igel" }).Options;
if (igel.SourceRef != "3.8.0" || igel.Pgo || igel.Architecture != hardware.IgelRecommended) throw new Exception("Wrong Igel defaults/hardware.");
igel.OutputRoot = Path.GetTempPath(); igel.Pgo = true;
try { core.Call(Request.Types.Operation.Plan, igel); throw new Exception("Unsupported Igel PGO accepted."); }
catch (InvalidOperationException e) when (e.Message.Contains("PGO")) { }
if (!Google.Protobuf.JsonParser.Default.Parse<BuildOptions>(Jupiter.ChessEngineCompiler.App.Services.ScriptExporter.SerializeOptions(igel)).Equals(igel)) throw new Exception("Igel script configuration did not round-trip.");
var caissa = core.Call(Request.Types.Operation.Defaults, new BuildOptions { Engine = "caissa" }).Options;
if (caissa.SourceRef != "2.0" || caissa.Architecture != hardware.CppRecommended || !hardware.CppChoices.Contains(caissa.Architecture)) throw new Exception("Wrong Caissa defaults/hardware.");
caissa.OutputRoot = Path.GetTempPath(); caissa.Pgo = true;
try { core.Call(Request.Types.Operation.Plan, caissa); throw new Exception("Unsupported Caissa PGO accepted."); }
catch (InvalidOperationException e) when (e.Message.Contains("PGO")) { }
core.Call(Request.Types.Operation.GithubStart, github: new GitHubQuery { Engine = "lc0", AuthMode = "invalid" });
var networkDeadline = DateTime.UtcNow.AddSeconds(5);
GitHubSnapshot network;
do { network = core.Call(Request.Types.Operation.GithubStatus).Github; if (network.State != "Running") break; Thread.Sleep(50); } while (DateTime.UtcNow < networkDeadline);
if (network.State != "Failed" || !network.Message.Contains("anonymous")) throw new Exception("Typed GitHub worker/error round-trip failed.");
var credentialFolder = Path.Combine(Path.GetTempPath(), "cec-credential-test-" + Guid.NewGuid().ToString("N"));
Directory.CreateDirectory(credentialFolder);
var preferences = Jupiter.ChessEngineCompiler.App.Services.ShellPreferences.Load(Path.Combine(credentialFolder, "ui.json"));
var credentials = new Jupiter.ChessEngineCompiler.App.Services.GitHubCredentials(credentialFolder, preferences);
try
{
    credentials.SaveToken("fixture-token-never-a-real-credential");
    var query = credentials.Prepare(new GitHubQuery());
    if (query.Token != "fixture-token-never-a-real-credential" || query.AuthMode != "token") throw new Exception("Windows credential round-trip failed.");
    if (File.ReadAllText(preferences.Path).Contains("fixture-token")) throw new Exception("Credential leaked into preferences.");
    query.Token = "";
}
finally { credentials.Disconnect(); Directory.Delete(credentialFolder, true); }
Console.WriteLine("Managed/native round-trip, result lifetime, validation and idle cancellation passed.");
if (args is ["--export-smoke", var app, var parent, var config])
{
    var options = Google.Protobuf.JsonParser.Default.Parse<BuildOptions>(File.ReadAllText(config));
    Console.WriteLine(Jupiter.ChessEngineCompiler.App.Services.ScriptExporter.Export(options, parent, app));
}
if (args is ["--github-smoke", var engine])
{
    core.Call(Request.Types.Operation.GithubStart, github: new GitHubQuery { Engine = engine, AuthMode = "cli", Action = GitHubQuery.Types.Action.Catalog, Page = 1 });
    do { network = core.Call(Request.Types.Operation.GithubStatus).Github; if (network.State != "Running") break; Thread.Sleep(150); } while (true);
    if (network.State != "Completed" || network.Releases.Count == 0 || network.Development.Count == 0) throw new Exception(network.Message);
    Console.WriteLine($"Managed/native GitHub catalog passed: {network.Releases.Count} releases, {network.Development.Count} development versions.");
}
