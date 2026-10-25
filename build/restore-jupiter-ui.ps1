param([string]$Destination)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$metadata = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot 'dependencies/jupiter-ui.json') | ConvertFrom-Json
$archive = Join-Path $PSScriptRoot ('dependencies/' + $metadata.archive)
$actual = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $metadata.sha256) { throw 'The pinned Jupiter.UI source archive does not match its SHA-256.' }
if (-not $Destination) { $Destination = Join-Path $root 'target/dependencies/jupiter-ui' }
$Destination = [IO.Path]::GetFullPath($Destination)
# Compare cached sources to archive bytes on every build. Never use the mutable
# sibling checkout or overwrite it, and never accept locally edited cache files.
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead($archive)
try {
    $entries = @($zip.Entries | Where-Object { $_.Name })
    if (-not (Test-Path -LiteralPath $Destination)) {
        New-Item -ItemType Directory -Path $Destination -Force | Out-Null
        [IO.Compression.ZipFile]::ExtractToDirectory($archive, $Destination)
    }
    foreach ($entry in $entries) {
        $file = [IO.Path]::GetFullPath((Join-Path $Destination $entry.FullName))
        if (-not $file.StartsWith($Destination + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid dependency archive path.' }
        if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Jupiter.UI cache is incomplete: $file. Use a fresh target/dependencies directory." }
        $stream = $entry.Open()
        try { $expected = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)) } finally { $stream.Dispose() }
        if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ne $expected) { throw "Jupiter.UI cache was changed: $file. Use a fresh target/dependencies directory." }
    }
    $expectedFiles = @($entries | ForEach-Object { $_.FullName.Replace('/', [IO.Path]::DirectorySeparatorChar) })
    foreach ($file in Get-ChildItem -LiteralPath $Destination -File -Recurse) {
        $relative = [IO.Path]::GetRelativePath($Destination, $file.FullName)
        # Build artifacts are the only permitted additions to the immutable source.
        if ($relative -match '(^|[\\/])(bin|obj)([\\/]|$)') { continue }
        if ($relative -notin $expectedFiles) { throw "Unexpected Jupiter.UI cached source: $file" }
    }
} finally { $zip.Dispose() }
Write-Host "Verified Jupiter.UI $($metadata.version), source commit $($metadata.commit), archive SHA-256 $actual"
