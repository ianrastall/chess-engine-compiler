param(
    [ValidateSet('Build','Plan','Doctor')][string]$Mode = 'Build',
    [string]$Config = (Join-Path $PSScriptRoot 'engine.json'),
    [string]$Compiler = (Join-Path $PSScriptRoot 'chess-engine-compiler.exe')
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $Compiler -PathType Leaf)) {
    throw 'chess-engine-compiler.exe is missing. Export build scripts from the GUI or pass -Compiler with its full path.'
}
if ($Mode -eq 'Doctor') {
    $engine = if (Test-Path -LiteralPath $Config -PathType Leaf) { (Get-Content -Raw -LiteralPath $Config | ConvertFrom-Json).engine } else { 'stockfish' }
    & $Compiler doctor --engine $engine
} else {
    if (-not (Test-Path -LiteralPath $Config -PathType Leaf)) { throw "Build configuration not found: $Config" }
    & $Compiler $Mode.ToLowerInvariant() $Config
}
exit $LASTEXITCODE
