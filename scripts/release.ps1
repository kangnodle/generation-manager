$ErrorActionPreference = 'Stop'

if ($env:OS -ne 'Windows_NT') {
    throw 'Run release.ps1 on Windows 11 with a default WSL 2 distribution.'
}

$wsl = (Get-Command wsl.exe -ErrorAction Stop).Source
$nsis = Get-Command makensis.exe -ErrorAction SilentlyContinue
$compiler = if ($nsis) {
    $nsis.Source
} else {
    Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe'
}
if (-not (Test-Path -LiteralPath $compiler -PathType Leaf)) {
    throw 'Install NSIS and add makensis.exe to PATH.'
}

$releaseDirectory = Join-Path $PSScriptRoot '../target/dist'
$null = New-Item -ItemType Directory -Path $releaseDirectory -Force
$staging = Join-Path $releaseDirectory ([guid]::NewGuid().ToString())
$staging = (New-Item -ItemType Directory -Path $staging).FullName
try {
    $scriptPath = & $wsl --exec wslpath -a -u (Join-Path $PSScriptRoot 'release.sh')
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $stagingPath = & $wsl --exec wslpath -a -u $staging
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & $wsl --exec bash $scriptPath --wsl $stagingPath
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $version, $architecture = Get-Content -LiteralPath (Join-Path $staging 'release-info.txt')
    $artifactName = "gm-$version-windows-wsl-$architecture"
    $launcherPath = Join-Path $staging 'gm.cmd'
    $installerPath = Join-Path $staging 'setup.nsi'

    Write-Host '==> Bundling'
    @'
@echo off
setlocal DisableDelayedExpansion
set "GM_WSL_BINARY=%~dp0gm-linux"
if defined WSLENV (set "WSLENV=%WSLENV%:GM_WSL_BINARY/p") else (set "WSLENV=GM_WSL_BINARY/p")
wsl.exe --exec bash -c "exec \"$GM_WSL_BINARY\" \"$@\"" gm %*
exit /b %ERRORLEVEL%
'@ | Set-Content -LiteralPath $launcherPath -Encoding Ascii
    $licensePath = Join-Path $staging 'LICENSE'
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot '../LICENSE') -Destination $licensePath
    $bundleFiles = $launcherPath, (Join-Path $staging 'gm-linux'), $licensePath
    $bundle = Join-Path $staging "$artifactName.zip"
    Compress-Archive -LiteralPath $bundleFiles -DestinationPath $bundle

    Write-Host '==> Packaging'
    @'
Unicode true
Name "Generation Manager (WSL)"
OutFile "gm-setup.exe"
InstallDir "$LOCALAPPDATA\Generation Manager"
InstallDirRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GenerationManager" "InstallLocation"
RequestExecutionLevel user
Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section
    SetOutPath "$INSTDIR"
    File "gm-linux"
    File "gm.cmd"
    File "LICENSE"
    WriteUninstaller "$INSTDIR\uninstall.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GenerationManager" "DisplayName" "Generation Manager (WSL)"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GenerationManager" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GenerationManager" "UninstallString" '"$INSTDIR\uninstall.exe"'
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\gm-linux"
    Delete "$INSTDIR\gm.cmd"
    Delete "$INSTDIR\LICENSE"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GenerationManager"
SectionEnd
'@ | Set-Content -LiteralPath $installerPath -Encoding Ascii
    & $compiler $installerPath
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    Move-Item -LiteralPath $bundle -Destination $releaseDirectory -Force
    Move-Item -LiteralPath (Join-Path $staging 'gm-setup.exe') `
        -Destination (Join-Path $releaseDirectory "$artifactName-setup.exe") -Force
    Write-Host "==> Release artifacts: $releaseDirectory"
} finally {
    Remove-Item -LiteralPath $staging -Recurse -Force
}
