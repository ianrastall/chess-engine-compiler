param([switch]$SkipChecks, [string]$InnoCompiler)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$version = ([xml](Get-Content -LiteralPath (Join-Path $root 'Directory.Build.props'))).Project.PropertyGroup.Version
if (-not $InnoCompiler) {
    $InnoCompiler = @(
        (Join-Path $env:LOCALAPPDATA 'Programs/Inno Setup 6/ISCC.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6/ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6/ISCC.exe'),
        (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source
    ) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) } | Select-Object -First 1
}
if (-not $InnoCompiler) { throw 'Inno Setup 6 is required. Every GUI release must include an installer. Install it or pass -InnoCompiler.' }
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
    # Recreate only this verified build output, so removed files cannot leak into installers.
    if ([IO.Path]::GetFullPath($app) -ne [IO.Path]::GetFullPath((Join-Path $root 'dist/ChessEngineCompiler'))) { throw 'Unexpected publish path.' }
    if (Test-Path -LiteralPath $app) { Remove-Item -LiteralPath $app -Recurse -Force }
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
    Invoke-Step { & $InnoCompiler /Qp "/DAppVersion=$version" (Join-Path $PSScriptRoot 'installer.iss') }
    $installer = Join-Path $root "dist/ChessEngineCompiler-$version-win-x64-setup.exe"
    if (-not (Test-Path -LiteralPath $installer)) { throw 'Required installer was not produced.' }
    $zip = Join-Path $root "dist/ChessEngineCompiler-$version-win-x64.zip"
    Compress-Archive -Path (Join-Path $app '*') -DestinationPath $zip -Force
    @($installer, $zip) | ForEach-Object {
        $hash = Get-FileHash -LiteralPath $_ -Algorithm SHA256
        "$($hash.Hash.ToLowerInvariant())  $([IO.Path]::GetFileName($_))"
    } | Set-Content -LiteralPath (Join-Path $root 'dist/SHA256SUMS.txt')
    Write-Host "Published installer: $installer"
    Write-Host "Supplemental portable package: $zip"
} finally { Pop-Location }
