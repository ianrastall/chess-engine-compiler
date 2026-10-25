param([switch]$SkipChecks, [string]$InnoCompiler)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
& (Join-Path $PSScriptRoot 'restore-jupiter-ui.ps1')
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
        Invoke-Step { dotnet build tests/LogViewTests -c Release }
        $logTestResult = Join-Path $root 'target/log-view-test.txt'
        if (Test-Path -LiteralPath $logTestResult) { Remove-Item -LiteralPath $logTestResult -Force }
        $logTest = Join-Path $root 'tests/LogViewTests/bin/x64/Release/net10.0-windows10.0.26100.0/win-x64/LogViewTests.exe'
        $testProcess = Start-Process -FilePath $logTest -ArgumentList ('"' + $logTestResult + '"') -PassThru -WindowStyle Hidden
        $testHandle = $testProcess.Handle
        if (-not $testProcess.WaitForExit(30000)) { $testProcess.Kill(); throw 'WinUI log viewport tests timed out.' }
        $testProcess.Refresh()
        if ($testProcess.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $logTestResult)) {
            $detail = if (Test-Path -LiteralPath $logTestResult) { Get-Content -Raw -LiteralPath $logTestResult } else { 'No test report was written.' }
            throw "WinUI log viewport tests failed (exit $($testProcess.ExitCode)): $detail"
        }
        $testReport = Get-Content -Raw -LiteralPath $logTestResult
        if (-not $testReport.StartsWith('PASS:')) { throw $testReport }
        Write-Host $testReport
    }
    Invoke-Step { cargo build -p chess-engine-compiler --release --locked --target-dir target }
    $app = Join-Path $root 'dist/ChessEngineCompiler'
    # Recreate only this verified build output, so removed files cannot leak into installers.
    if ([IO.Path]::GetFullPath($app) -ne [IO.Path]::GetFullPath((Join-Path $root 'dist/ChessEngineCompiler'))) { throw 'Unexpected publish path.' }
    if (Test-Path -LiteralPath $app) { Remove-Item -LiteralPath $app -Recurse -Force }
    Invoke-Step { dotnet publish src/ChessEngineCompiler.App -c Release --artifacts-path (Join-Path $root 'target/managed-artifacts') -o $app }
    if (-not $SkipChecks) {
        $uiTestReport = Join-Path $root 'target/engine-ui-test/report.txt'
        New-Item -ItemType Directory -Path (Split-Path $uiTestReport -Parent) -Force | Out-Null
        if (Test-Path -LiteralPath $uiTestReport) { Remove-Item -LiteralPath $uiTestReport }
        $uiTest = Start-Process -FilePath (Join-Path $app 'ChessEngineCompiler.exe') -ArgumentList ('--verify-ui "' + $uiTestReport + '"') -PassThru -WindowStyle Hidden
        $uiHandle = $uiTest.Handle
        if (-not $uiTest.WaitForExit(45000)) { $uiTest.Kill(); throw 'WinUI engine form verification timed out.' }
        $uiTest.Refresh()
        if ($uiTest.ExitCode -ne 0) { throw "WinUI engine form verification exited with $($uiTest.ExitCode)." }
        if (-not (Test-Path -LiteralPath $uiTestReport)) { throw 'WinUI engine form verification produced no report.' }
        $uiReport = Get-Content -Raw -LiteralPath $uiTestReport
        if (-not $uiReport.StartsWith('PASS:')) { throw $uiReport }
        Write-Host $uiReport
    }
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
    foreach ($name in 'ChessEngineCompiler.exe','ChessEngineCompiler.pri','compiler_ffi.dll','chess-engine-compiler.exe','vcruntime140.dll','Assets/app.ico') {
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
