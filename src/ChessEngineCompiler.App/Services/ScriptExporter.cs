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
        backend = o.Backend,
        pythonPath = o.PythonPath,
        visualStudio = o.VisualStudio,
        cudaRoot = o.CudaRoot,
        weightsPath = o.WeightsPath,
        autoArchitecture = o.AutoArchitecture,
        zigPath = o.ZigPath,
    }, new JsonSerializerOptions { WriteIndented = true });

    public static string Export(BuildOptions options, string parent, string appDirectory)
    {
        if (options.Engine is not ("stockfish" or "lc0" or "berserk" or "obsidian" or "reckless" or "igel" or "caissa" or "alexandria" or "avalanche")) throw new ArgumentException("Unsupported engine.", nameof(options));
        var folder = Path.Combine(parent, $"{options.Engine}-scripts-{DateTime.Now:yyyyMMdd-HHmmss}-{Guid.NewGuid():N}");
        Directory.CreateDirectory(folder);
        File.WriteAllText(Path.Combine(folder, "engine.json"), SerializeOptions(options));
        File.WriteAllText(Path.Combine(folder, $"{options.Engine}.json"), SerializeOptions(options));
        foreach (var file in Directory.GetFiles(Path.Combine(appDirectory, "scripts")))
        {
            if (options.Engine != "stockfish" && Path.GetFileName(file) == "build-stockfish.cmd") continue;
            File.Copy(file, Path.Combine(folder, Path.GetFileName(file)));
        }
        // Keep the original Stockfish entry point and provide the same alias for the other engines.
        File.Copy(Path.Combine(folder, "build-engine.cmd"), Path.Combine(folder, $"build-{options.Engine}.cmd"), true);
        File.WriteAllText(Path.Combine(folder, "check-tools.cmd"), $"@echo off\r\n\"%~dp0chess-engine-compiler.exe\" doctor --engine {options.Engine}\r\nexit /b %errorlevel%\r\n");
        File.Copy(Path.Combine(appDirectory, "chess-engine-compiler.exe"), Path.Combine(folder, "chess-engine-compiler.exe"));
        foreach (var file in Directory.GetFiles(appDirectory, "vcruntime*.dll")) File.Copy(file, Path.Combine(folder, Path.GetFileName(file)));
        return folder;
    }
}
