# Install a published Windows x64 release. No Rust or Python required.
& {
    $ErrorActionPreference = 'Stop'
    Set-StrictMode -Version Latest

    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        throw 'Use install.sh on macOS or Linux.'
    }
    if ($env:PROCESSOR_ARCHITECTURE -notin @('AMD64', 'ARM64') -and $env:PROCESSOR_ARCHITEW6432 -ne 'AMD64') {
        throw 'pomo requires 64-bit Windows.'
    }

    $installDir = if ($env:POMO_INSTALL_DIR) { $env:POMO_INSTALL_DIR } else {
        Join-Path $env:LOCALAPPDATA 'Programs\pomo'
    }
    if (-not [IO.Path]::IsPathRooted($installDir)) { throw 'POMO_INSTALL_DIR must be absolute.' }
    $base = 'https://github.com/AzAINN/Pomodoro/releases/latest/download'
    if ($env:POMO_VERSION) {
        if ($env:POMO_VERSION -notmatch '^v[0-9][a-zA-Z0-9.+-]*$') { throw 'Use a release tag such as v0.2.0.' }
        $base = "https://github.com/AzAINN/Pomodoro/releases/download/$env:POMO_VERSION"
    }
    $archive = 'pomo-x86_64-pc-windows-msvc.zip'
    $work = Join-Path ([IO.Path]::GetTempPath()) ('pomo-install-' + [Guid]::NewGuid().ToString('N'))
    $staged = $null
    New-Item -ItemType Directory -Path $work | Out-Null
    $previousProtocol = [Net.ServicePointManager]::SecurityProtocol
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$archive" -OutFile (Join-Path $work $archive)
        Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile (Join-Path $work 'SHA256SUMS')
        $lines = @(Get-Content (Join-Path $work 'SHA256SUMS') | Where-Object {
            $_ -match ('^([a-fA-F0-9]{64})\s+' + [regex]::Escape($archive) + '$')
        })
        if ($lines.Count -ne 1) { throw 'Missing or malformed release checksum.' }
        $expected = ($lines[0] -split '\s+')[0]
        $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $work $archive)).Hash
        if ($actual -ne $expected) { throw 'Checksum mismatch; nothing was installed.' }

        # Extract only the expected file, not arbitrary archive paths.
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $zip = [IO.Compression.ZipFile]::OpenRead((Join-Path $work $archive))
        try {
            $entry = $zip.GetEntry('pomo.exe')
            if ($null -eq $entry) { throw 'Release has no pomo.exe.' }
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, (Join-Path $work 'pomo.exe'))
        } finally { $zip.Dispose() }

        New-Item -ItemType Directory -Force -Path $installDir | Out-Null
        $destination = Join-Path $installDir 'pomo.exe'
        if (Test-Path -LiteralPath $destination -PathType Container) { throw "$destination is a directory." }
        $staged = Join-Path $installDir ('.pomo-' + [Guid]::NewGuid().ToString('N') + '.exe')
        Copy-Item -LiteralPath (Join-Path $work 'pomo.exe') -Destination $staged
        Move-Item -Force -LiteralPath $staged -Destination $destination
        $staged = $null

        $userPath = [string][Environment]::GetEnvironmentVariable('Path', 'User')
        if ($installDir -notin ($userPath -split ';')) {
            [Environment]::SetEnvironmentVariable('Path', ($userPath.TrimEnd(';') + ';' + $installDir).TrimStart(';'), 'User')
        }
        if ($installDir -notin ($env:PATH -split ';')) { $env:PATH += ";$installDir" }
        Write-Host "Installed $destination. Run: pomo"
    } finally {
        [Net.ServicePointManager]::SecurityProtocol = $previousProtocol
        if ($staged -and (Test-Path -LiteralPath $staged)) { Remove-Item -Force -LiteralPath $staged }
        Remove-Item -Recurse -Force -LiteralPath $work
    }
}
