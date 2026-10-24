using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;

namespace Jupiter.ChessEngineCompiler.App;

public static partial class Program
{
    [STAThread]
    public static void Main()
    {
        // The installer checks for this named object before upgrading/uninstalling.
        using var installationMutex = new Mutex(false, @"Local\Jupiter.ChessEngineCompiler");
        System.Runtime.InteropServices.Marshal.ThrowExceptionForHR(SetCurrentProcessExplicitAppUserModelID("Jupiter.ChessEngineCompiler"));
        WinRT.ComWrappersSupport.InitializeComWrappers();
        Application.Start(callback =>
        {
            SynchronizationContext.SetSynchronizationContext(new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread()));
            _ = new App();
        });
        GC.KeepAlive(installationMutex);
    }

    [System.Runtime.InteropServices.LibraryImport("shell32.dll", StringMarshalling = System.Runtime.InteropServices.StringMarshalling.Utf16)]
    private static partial int SetCurrentProcessExplicitAppUserModelID(string appId);
}
