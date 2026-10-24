using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Google.Protobuf;
using Jupiter.ChessEngineCompiler.Messages;

[assembly: DefaultDllImportSearchPaths(DllImportSearchPath.AssemblyDirectory | DllImportSearchPath.System32)]
[assembly: DisableRuntimeMarshalling]

namespace Jupiter.ChessEngineCompiler.Interop;

internal static unsafe partial class Native
{
    private const string Library = "compiler_ffi";
    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint cec_abi_version();
    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint cec_create(out CoreHandle core);
    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint cec_call(CoreHandle core, byte* request, nuint length, out ResultHandle result);
    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint cec_result_bytes(ResultHandle result, out byte* bytes, out nuint length);
    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void cec_result_free(nint result);
    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void cec_destroy(nint core);
}
internal sealed class CoreHandle() : SafeHandle(nint.Zero, true)
{
    public override bool IsInvalid => handle == nint.Zero;
    protected override bool ReleaseHandle() { Native.cec_destroy(handle); return true; }
}
internal sealed class ResultHandle() : SafeHandle(nint.Zero, true)
{
    public override bool IsInvalid => handle == nint.Zero;
    protected override bool ReleaseHandle() { Native.cec_result_free(handle); return true; }
    internal unsafe byte[] Copy()
    {
        if (Native.cec_result_bytes(this, out var bytes, out var length) != 0 || length > int.MaxValue)
            throw new InvalidOperationException("Could not read the native build result.");
        return new ReadOnlySpan<byte>(bytes, (int)length).ToArray();
    }
}
public sealed class CompilerCore : IDisposable
{
    private readonly CoreHandle _handle;
    private ulong _requestId;
    public CompilerCore()
    {
        if (Native.cec_abi_version() != 1) throw new InvalidOperationException("Native core version mismatch.");
        if (Native.cec_create(out _handle) != 0) throw new InvalidOperationException("Could not initialize the build core.");
    }
    public unsafe Response Call(Request.Types.Operation operation, BuildOptions? options = null)
    {
        var request = new Request { RequestId = ++_requestId, Operation = operation, Options = options };
        var bytes = request.ToByteArray();
        fixed (byte* pointer = bytes)
        {
            var code = Native.cec_call(_handle, pointer, (nuint)bytes.Length, out var result);
            using (result)
            {
                if (code != 0) throw new InvalidOperationException($"Build core call failed ({code}).");
                var response = Response.Parser.ParseFrom(result.Copy());
                if (response.RequestId != request.RequestId) throw new InvalidOperationException("Build response ID mismatch.");
                if (response.Error.Length > 0) throw new InvalidOperationException(response.Error);
                return response;
            }
        }
    }
    public void Dispose() => _handle.Dispose();
}
