using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using Jupiter.ChessEngineCompiler.Messages;

namespace Jupiter.ChessEngineCompiler.App.Services;

/// <summary>Only the selected authentication mode is a presentation preference.
/// The credential itself belongs to Windows Credential Manager, never JSON.</summary>
public sealed class GitHubCredentials(string dataFolder, ShellPreferences preferences)
{
    private readonly string _target = "Jupiter.ChessEngineCompiler.GitHub." + Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(Path.GetFullPath(dataFolder).ToUpperInvariant())))[..24];
    public string Mode => preferences.GitHubAuthMode;
    public bool Connected => Mode is "token" or "cli";
    public void SelectMode(string mode) { preferences.GitHubAuthMode = mode; preferences.Save(); }
    public GitHubQuery Prepare(GitHubQuery query)
    {
        var copy = query.Clone(); copy.AuthMode = Mode;
        if (Mode == "token") copy.Token = Read() ?? throw new InvalidOperationException("The saved GitHub token is unavailable. Save it again in Settings > GitHub.");
        return copy;
    }
    public void SaveToken(string token)
    {
        if (string.IsNullOrWhiteSpace(token) || token.Length > 1000) throw new ArgumentException("Enter a GitHub personal access token.");
        var bytes = Encoding.Unicode.GetBytes(token);
        var pointer = Marshal.AllocHGlobal(bytes.Length);
        try
        {
            Marshal.Copy(bytes, 0, pointer, bytes.Length);
            var credential = new Credential { Type = 1, TargetName = _target, UserName = "GitHub", BlobSize = (uint)bytes.Length, Blob = pointer, Persist = 2 };
            if (!CredWrite(ref credential, 0)) throw new Win32Exception(Marshal.GetLastWin32Error(), "Windows could not save the GitHub credential.");
            SelectMode("token");
        }
        finally { CryptographicOperations.ZeroMemory(bytes); for (var i = 0; i < bytes.Length; i++) Marshal.WriteByte(pointer, i, 0); Marshal.FreeHGlobal(pointer); }
    }
    private string? Read()
    {
        if (!CredRead(_target, 1, 0, out var pointer)) return null;
        try { var credential = Marshal.PtrToStructure<Credential>(pointer); return Marshal.PtrToStringUni(credential.Blob, checked((int)credential.BlobSize / 2)); }
        finally { CredFree(pointer); }
    }
    public void Disconnect()
    {
        if (!CredDelete(_target, 1, 0) && Marshal.GetLastWin32Error() != 1168) throw new Win32Exception(Marshal.GetLastWin32Error(), "Windows could not remove the saved GitHub credential.");
        SelectMode("anonymous");
    }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct Credential
    {
        public uint Flags, Type;
        public string TargetName;
        public string? Comment;
        public System.Runtime.InteropServices.ComTypes.FILETIME LastWritten;
        public uint BlobSize;
        public nint Blob;
        public uint Persist, AttributeCount;
        public nint Attributes;
        public string? TargetAlias;
        public string UserName;
    }
    [DllImport("advapi32.dll", EntryPoint = "CredWriteW", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)] private static extern bool CredWrite(ref Credential credential, uint flags);
    [DllImport("advapi32.dll", EntryPoint = "CredReadW", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)] private static extern bool CredRead(string target, uint type, uint flags, out nint credential);
    [DllImport("advapi32.dll", EntryPoint = "CredDeleteW", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)] private static extern bool CredDelete(string target, uint type, uint flags);
    [DllImport("advapi32.dll")] private static extern void CredFree(nint credential);
}
