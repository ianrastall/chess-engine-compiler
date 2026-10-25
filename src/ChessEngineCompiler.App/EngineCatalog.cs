namespace Jupiter.ChessEngineCompiler.App;

internal sealed record EngineEntry(string Id, string Name);
internal static class EngineCatalog
{
    internal static readonly EngineEntry[] Items = new EngineEntry[]
    {
        new("stockfish", "Stockfish"), new("lc0", "Lc0"), new("berserk", "Berserk"),
        new("obsidian", "Obsidian"), new("reckless", "Reckless"), new("igel", "Igel"), new("caissa", "Caissa"), new("alexandria", "Alexandria"),
    }.OrderBy(e => e.Name, StringComparer.OrdinalIgnoreCase).ToArray();
    internal static int Index(string id) => Array.FindIndex(Items, e => e.Id == id);
    internal static string Name(string id) => Items.FirstOrDefault(e => e.Id == id)?.Name ?? id;
}
