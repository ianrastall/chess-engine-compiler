param([switch]$SkipChecks)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
function Invoke-Step([scriptblock]$Command) {
    & $Command
    if ($LASTEXITCODE -ne 0) { throw "Build step failed (exit $LASTEXITCODE)." }
}
Push-Location $root
try {
    if (-not $SkipChecks) {
        Invoke-Step { cargo fmt --all --check }
        Invoke-Step { cargo clippy --workspace --all-targets --locked -- -D warnings }
        Invoke-Step { cargo test --workspace --locked }
        Invoke-Step { dotnet run --project tests/InteropTests -c Release }
    }
    Invoke-Step { cargo build -p chess-engine-compiler --release --locked --target-dir target }
    $app = Join-Path $root 'dist/ChessEngineCompiler'
    Invoke-Step { dotnet publish src/ChessEngineCompiler.App -c Release -o $app }
    Copy-Item -LiteralPath (Join-Path $root 'target/x86_64-pc-windows-msvc/release/chess-engine-compiler.exe') -Destination $app -Force
    $scripts = Join-Path $app 'scripts'
    New-Item -ItemType Directory -Path $scripts -Force | Out-Null
    Get-ChildItem -LiteralPath (Join-Path $root 'scripts') -File | Copy-Item -Destination $scripts -Force
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    $crt = Get-ChildItem -Path (Join-Path $vs 'VC/Redist/MSVC/*/x64/Microsoft.VC*.CRT') -Directory | Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $crt) { throw 'MSVC x64 redistributable files were not found.' }
    Get-ChildItem -LiteralPath $crt.FullName -Filter '*.dll' | Copy-Item -Destination $app -Force
    Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination $app -Force
    foreach ($name in 'ChessEngineCompiler.exe','ChessEngineCompiler.pri','compiler_ffi.dll','chess-engine-compiler.exe','vcruntime140.dll') {
        if (-not (Test-Path -LiteralPath (Join-Path $app $name))) { throw "Missing published file: $name" }
    }
    $zip = Join-Path $root 'dist/ChessEngineCompiler-0.1.0-win-x64.zip'
    Compress-Archive -Path (Join-Path $app '*') -DestinationPath $zip -Force
    Write-Host "Published $app and $zip"
} finally { Pop-Location }
