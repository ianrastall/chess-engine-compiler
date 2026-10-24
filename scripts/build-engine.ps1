param(
    [ValidateSet('Build','Plan','Doctor')][string]$Mode = 'Build',
    [string]$Config = (Join-Path $PSScriptRoot 'stockfish.json'),
    [string]$Compiler = (Join-Path $PSScriptRoot 'chess-engine-compiler.exe')
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $Compiler -PathType Leaf)) {
    throw 'chess-engine-compiler.exe is missing. Export build scripts from the GUI or pass -Compiler with its full path.'
}
if ($Mode -eq 'Doctor') {
    & $Compiler doctor
} else {
    if (-not (Test-Path -LiteralPath $Config -PathType Leaf)) { throw "Build configuration not found: $Config" }
    & $Compiler $Mode.ToLowerInvariant() $Config
}
exit $LASTEXITCODE
