# GitHub versions, access and downloads

## Compile source

The Compile work area loads official releases and development versions separately for the selected engine. Official releases exclude GitHub prereleases. Development versions combine recent commits from the repository's default branch and published prereleases, sorted newest first. Commit selection pins an exact SHA; release/prerelease selection uses its tag. **Custom tag, branch or commit** accepts refs such as `master` or a specific commit. Refresh and Newer/Older fetch pages of metadata; they do not compile or download source. Failed metadata requests leave custom refs available.

The supported repositories are `official-stockfish/Stockfish` and `LeelaChessZero/lc0`. The adapters are verified against current releases and development source; historical upstream releases can require older compilers or build systems outside the current adapters.

CPU detection reads CPUID and Rust's OS-aware feature checks. Only compatible Stockfish targets appear. Automatic selection prefers available VNNI/AVX-512/AVX2/BMI2 variants, while older AMD Zen processors avoid slow PEXT. This is a feature-based recommendation, not a guarantee of maximum nodes per second. Manual selection remains available. Lc0 uses its own MSVC CPU or CUDA GPU workflow; Stockfish CPU presets are not applied to it.

## Connect an account

Open **Settings** (File > Settings, Ctrl+, or the navigation footer) and select **Manage GitHub access…** under GitHub:

A completed account check displays **Connection test successful**, followed by the account and sign-in method. It confirms that settings are saved and the dialog can be closed. Cancelled and failed checks are explicitly labelled.

- **Anonymous:** normal public release and source browsing works without an account, within GitHub's unauthenticated rate limits.
- **Personal access token:** paste into the masked password field and choose Validate and save token. The app validates the account before replacing the stored credential. Windows Credential Manager stores the secret; ui.json contains only the selected authentication mode. The saved token is represented by a masked placeholder and is never revealed, exported, placed on process command lines or written into build logs.
- **GitHub CLI account:** choose Use GitHub CLI account to validate and use its existing signed-in account. The app reads the credential privately when needed; it does not copy it into its own settings or token store.
- **Browser sign-in:** install [GitHub CLI](https://cli.github.com/) if needed, then choose Sign in through browser. GitHub CLI starts its device-code flow and opens GitHub. Follow the code and authorization instructions displayed in the dialog. This signs GitHub CLI in for this Windows user. Cancel sign-in or close the dialog to stop the pending operation. No separate OAuth app registration is required by this application.

Tokens/accounts must have access to the requested repositories and API resources. GitHub documents **Actions read** permission for fine-grained workflow-artifact access; classic OAuth/PAT artifact downloads require the `repo` scope. Public release downloads can remain anonymous. Token types and organization policies can impose additional restrictions. The app reports authentication, permission, rate-limit and expiry errors rather than treating an inaccessible artifact as downloadable. See [GitHub artifact API permissions](https://docs.github.com/en/rest/actions/artifacts) and [CLI browser sign-in](https://cli.github.com/manual/gh_auth_login).

**Disconnect this app** removes its saved personal token and selects anonymous mode. It does not sign GitHub CLI out of other applications. The normal installer preserves settings and credentials on upgrade/uninstall; use Disconnect first if you want to remove the saved token.

## Download existing files

The **Downloads** work area has its own engine selection, destination, metadata loader and download worker. It can operate alongside compilation. **Release assets** lists all files attached to the chosen official release or prerelease, including binaries, checksums or source archives. Choose the file matching your operating system and CPU/GPU. **Workflow artifacts** requires an authenticated account/token and displays branch/commit context, creation/expiry and size. Expired artifacts cannot be downloaded. Newer/Older page through the available files; release paging is separate.

Each download creates a new uniquely named destination subfolder. A temporary file is renamed only after the transfer is complete and any published SHA-256 matches. `download.json` records repository, asset/artifact ID, stable API source URL, bytes and computed/published digests. Signed redirect URLs and credentials are never recorded. Cancelling removes the incomplete temporary file and preserves existing downloads. The application does not extract, install or execute downloaded files.

Metadata requests have finite timeouts and retry transient GitHub failures. If GitHub's repository-wide artifact list fails, the adapter can fall back to paginated workflow-run artifact lists. Download streaming supports cancellation even while a connection is stalled. HTTPS redirects are checked, and credentials are stripped when a download redirects to a different host.

## Logs and PowerShell

**Copy log** copies the complete saved build log, including lines outside the live 300-line tail. When a preview is showing, it copies that preview. Normal text selection remains available. Highlighting changes presentation only: commands/string arguments, warnings and errors use distinct colours; saved/copied text stays plain. Contrast themes use their native text colour.

Build tools run through PowerShell 7 when available. Executable paths and argument arrays are passed as JSON data, without shell interpolation. PowerShell output is kept plain for saving and rendered with highlights by WinUI. Systems without PowerShell 7 use direct process execution. Interactive UCI stdin is always direct. **View > Watch log in PowerShell** opens a separate full-log viewer that follows new lines with diagnostic colours. This does not repeat build commands.

## CLI

```powershell
.\chess-engine-compiler.exe hardware
.\chess-engine-compiler.exe github stockfish --action catalog
.\chess-engine-compiler.exe github lc0 --action catalog --auth cli
.\chess-engine-compiler.exe github stockfish --action artifacts --auth cli
.\chess-engine-compiler.exe github stockfish --action assets --id <release-id>
.\chess-engine-compiler.exe github stockfish --action download-release --id <asset-id> --output "D:\Downloads"
.\chess-engine-compiler.exe github stockfish --action download-artifact --id <artifact-id> --auth cli --output "D:\Downloads"
```

Use `--page` for metadata pagination. CLI authentication is anonymous or explicitly selected GitHub CLI; no token argument is accepted. JSON results go to stdout; failure diagnostics go to stderr.
