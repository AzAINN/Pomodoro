# Run on Windows; requests are mocked and the original PATH is restored.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('pomo-installer-test-' + [Guid]::NewGuid().ToString('N'))
$savedUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$savedPath = $env:PATH
$savedInstallDir = $env:POMO_INSTALL_DIR
$savedVersion = $env:POMO_VERSION
New-Item -ItemType Directory -Path $testRoot | Out-Null

function Invoke-WebRequest {
    param([switch]$UseBasicParsing, [string]$Uri, [string]$OutFile)
    if (-not $Uri.StartsWith('https://github.com/AzAINN/Pomodoro/releases/download/v0.2.0/')) {
        throw "Unexpected download URL: $Uri"
    }
    Copy-Item -LiteralPath (Join-Path $testRoot ([Uri]$Uri).Segments[-1]) -Destination $OutFile
}

try {
    $fixture = Join-Path $testRoot 'pomo.exe'
    [IO.File]::WriteAllText($fixture, 'pomo release fixture')
    $archive = Join-Path $testRoot 'pomo-x86_64-pc-windows-msvc.zip'
    Compress-Archive -LiteralPath $fixture -DestinationPath $archive
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant()
    $checksum = Join-Path $testRoot 'SHA256SUMS'
    [IO.File]::WriteAllText($checksum, "$hash  pomo-x86_64-pc-windows-msvc.zip`n")
    $env:POMO_VERSION = 'v0.2.0'
    $env:POMO_INSTALL_DIR = Join-Path $testRoot 'directory with spaces'
    & (Join-Path $root 'install.ps1')
    $installed = Join-Path $env:POMO_INSTALL_DIR 'pomo.exe'
    if ([IO.File]::ReadAllText($installed) -ne 'pomo release fixture') { throw 'Incorrect installed binary.' }
    if ($env:POMO_INSTALL_DIR -notin ($env:PATH -split ';')) { throw 'PATH was not updated.' }

    [IO.File]::WriteAllText($installed, 'existing installation')
    [IO.File]::WriteAllText($checksum, ('0' * 64) + "  pomo-x86_64-pc-windows-msvc.zip`n")
    $failed = $false
    try { & (Join-Path $root 'install.ps1') } catch { $failed = $true }
    if (-not $failed) { throw 'A checksum mismatch was accepted.' }
    if ([IO.File]::ReadAllText($installed) -ne 'existing installation') { throw 'Existing binary was changed after failure.' }
    Write-Host 'Windows installer passed: checksum, install path, PATH, and failed-update preservation.'
} finally {
    [Environment]::SetEnvironmentVariable('Path', $savedUserPath, 'User')
    $env:PATH = $savedPath
    $env:POMO_INSTALL_DIR = $savedInstallDir
    $env:POMO_VERSION = $savedVersion
    Remove-Item -Recurse -Force -LiteralPath $testRoot
}
