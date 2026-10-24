using CommunityToolkit.Mvvm.ComponentModel;

namespace Jupiter.ChessEngineCompiler.App;

public partial class BuildViewModel : ObservableObject
{
    [ObservableProperty] public partial string SourceRef { get; set; } = "sf_19";
    [ObservableProperty] public partial string MsysRoot { get; set; } = "";
    [ObservableProperty] public partial string OutputRoot { get; set; } = "";
    [ObservableProperty] public partial string Status { get; set; } = "Ready — configure an engine build.";
}
