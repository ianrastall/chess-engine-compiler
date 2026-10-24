param([string]$Installer)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$version = ([xml](Get-Content -LiteralPath (Join-Path $root 'Directory.Build.props'))).Project.PropertyGroup.Version
if (-not $Installer) { $Installer = Join-Path $root "dist/ChessEngineCompiler-$version-win-x64-setup.exe" }
$registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Jupiter.ChessEngineCompiler_is1'
$app = Join-Path $env:LOCALAPPDATA 'Programs/Jupiter/ChessEngineCompiler'
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Chess Engine Compiler.lnk'
$data = Join-Path $env:LOCALAPPDATA 'Jupiter/ChessEngineCompiler'
if ((Test-Path -LiteralPath $app) -or (Test-Path -LiteralPath $registration) -or (Test-Path -LiteralPath $shortcut)) {
    throw 'Installation verification requires no pre-existing Chess Engine Compiler installation. Run the installer normally to upgrade an existing app.'
}
if (Get-Process ChessEngineCompiler -ErrorAction SilentlyContinue) { throw 'Close Chess Engine Compiler before installation verification.' }
New-Item -ItemType Directory -Path (Join-Path $root 'target/install-validation') -Force | Out-Null
$originals = @{}
foreach ($name in 'ui.json', 'build-options.json') {
    $file = Join-Path $data $name
    if (Test-Path -LiteralPath $file) { $originals[$file] = (Get-FileHash -LiteralPath $file).Hash }
}
function Check-UserData {
    foreach ($file in $originals.Keys) {
        if (-not (Test-Path -LiteralPath $file) -or (Get-FileHash -LiteralPath $file).Hash -ne $originals[$file]) { throw "User data changed: $file" }
    }
}
function Run-Installer([string]$Label) {
    $log = Join-Path $root "target/install-validation/$Label.log"
    $process = Start-Process -FilePath $Installer -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/SP-',"/LOG=`"$log`"") -Wait -PassThru -WindowStyle Hidden
    if ($process.ExitCode -ne 0) { throw "$Label failed: exit $($process.ExitCode). See $log" }
    if (-not (Test-Path -LiteralPath (Join-Path $app 'ChessEngineCompiler.exe')) -or -not (Test-Path -LiteralPath $shortcut)) { throw 'Installed app or Start Menu entry is missing.' }
    if ((Get-ItemProperty -LiteralPath $registration).DisplayVersion -ne $version) { throw 'Installed version mismatch.' }
    $shell = New-Object -ComObject WScript.Shell
    $link = $shell.CreateShortcut($shortcut)
    if ($link.TargetPath -ne (Join-Path $app 'ChessEngineCompiler.exe')) { throw 'Start Menu shortcut does not target the installed GUI.' }
    Check-UserData
    Write-Output "$Label passed: installed files, version, Start Menu target and preserved user data."
}
Run-Installer 'install'
Run-Installer 'upgrade'
$uninstaller = Join-Path $app 'unins000.exe'
$process = Start-Process -FilePath $uninstaller -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART') -Wait -PassThru -WindowStyle Hidden
if ($process.ExitCode -ne 0) { throw 'Uninstall failed.' }
if ((Test-Path -LiteralPath (Join-Path $app 'ChessEngineCompiler.exe')) -or (Test-Path -LiteralPath $registration) -or (Test-Path -LiteralPath $shortcut)) { throw 'Uninstall left the app, registry entry or shortcut.' }
Check-UserData
Write-Output 'Uninstall passed; user data is unchanged.'
Run-Installer 'final-install'
& (Join-Path $app 'chess-engine-compiler.exe') doctor
if ($LASTEXITCODE -ne 0) { throw 'Installed CLI smoke test failed.' }
Write-Output "Installed and ready: $shortcut"
