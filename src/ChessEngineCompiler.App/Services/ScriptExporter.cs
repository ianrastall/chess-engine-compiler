using System.Text.Json;
using Jupiter.ChessEngineCompiler.Messages;

namespace Jupiter.ChessEngineCompiler.App.Services;

public static class ScriptExporter
{
    public static string SerializeOptions(BuildOptions o) => JsonSerializer.Serialize(new
    {
        engine = o.Engine,
        sourceRef = o.SourceRef,
        msysRoot = o.MsysRoot,
        outputRoot = o.OutputRoot,
        architecture = o.Architecture,
        jobs = o.Jobs,
        pgo = o.Pgo,
        strip = o.Strip,
    }, new JsonSerializerOptions { WriteIndented = true });

    public static string Export(BuildOptions options, string parent, string appDirectory)
    {
        var folder = Path.Combine(parent, $"stockfish-scripts-{DateTime.Now:yyyyMMdd-HHmmss}-{Guid.NewGuid():N}");
        Directory.CreateDirectory(folder);
        File.WriteAllText(Path.Combine(folder, "stockfish.json"), SerializeOptions(options));
        foreach (var file in Directory.GetFiles(Path.Combine(appDirectory, "scripts"))) File.Copy(file, Path.Combine(folder, Path.GetFileName(file)));
        File.Copy(Path.Combine(appDirectory, "chess-engine-compiler.exe"), Path.Combine(folder, "chess-engine-compiler.exe"));
        foreach (var file in Directory.GetFiles(appDirectory, "vcruntime*.dll")) File.Copy(file, Path.Combine(folder, Path.GetFileName(file)));
        return folder;
    }
}
