namespace Jupiter.ChessEngineCompiler.App;

internal sealed record EngineEntry(string Id, string Name);
internal static class EngineCatalog
{
    internal static readonly EngineEntry[] LibraryOrder =
    [
        new("stockfish", "Stockfish"), new("lc0", "Lc0"), new("obsidian", "Obsidian"),
    ];
    internal static readonly EngineEntry[] Items =
        LibraryOrder.OrderBy(e => e.Name, StringComparer.OrdinalIgnoreCase).ToArray();
    internal static int Index(string id) => Array.FindIndex(Items, e => e.Id == id);
    internal static string Name(string id) => Items.FirstOrDefault(e => e.Id == id)?.Name ?? id;
}
