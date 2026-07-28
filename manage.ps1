$ErrorActionPreference = 'Stop'

$command = if ($args.Length -gt 0) { $args[0] } else { 'install' }
$remaining = if ($args.Length -gt 1) { $args[1..($args.Length - 1)] } else { @() }
if ($command -in @('-h', '--help', 'help')) {
    $command = 'install'
    $remaining = @('--help')
}

$channel = if ($env:CONCORD_CHANNEL) { $env:CONCORD_CHANNEL } else { 'stable' }
$version = if ($env:CONCORD_VERSION) { $env:CONCORD_VERSION } else { '' }
$publicUrl = if ($env:CONCORD_RELEASES_PUBLIC_URL) { $env:CONCORD_RELEASES_PUBLIC_URL } else { 'https://releases.concord.perish.uk' }
$defaultInstall = if ($env:LOCALAPPDATA) { Join-Path $env:LOCALAPPDATA 'concord' } elseif ($env:HOME) { Join-Path $env:HOME '.local/share/concord' } else { '.' }
$defaultBin = if ($env:USERPROFILE) { Join-Path $env:USERPROFILE '.local\bin' } elseif ($env:HOME) { Join-Path $env:HOME '.local/bin' } else { '.' }
$installRoot = if ($env:CONCORD_INSTALL_ROOT) { $env:CONCORD_INSTALL_ROOT } else { $defaultInstall }
$localBinDir = if ($env:CONCORD_LOCAL_BIN_DIR) { $env:CONCORD_LOCAL_BIN_DIR } else { $defaultBin }
$retain = if ($env:CONCORD_RETAIN) { $env:CONCORD_RETAIN } else { 'true' }
$marker = 'concord-manager-v1'

for ($i = 0; $i -lt $remaining.Length; $i++) {
    $arg = $remaining[$i]
    switch -Regex ($arg) {
        '^--channel$' { $i++; $channel = $remaining[$i]; continue }
        '^--channel=(.+)$' { $channel = $Matches[1]; continue }
        '^--version$' { $i++; $version = $remaining[$i]; continue }
        '^--version=(.+)$' { $version = $Matches[1]; continue }
        '^--public-url$' { $i++; $publicUrl = $remaining[$i]; continue }
        '^--public-url=(.+)$' { $publicUrl = $Matches[1]; continue }
        '^--install-root$' { $i++; $installRoot = $remaining[$i]; continue }
        '^--install-root=(.+)$' { $installRoot = $Matches[1]; continue }
        '^--bin-dir$' { $i++; $localBinDir = $remaining[$i]; continue }
        '^--bin-dir=(.+)$' { $localBinDir = $Matches[1]; continue }
        '^--retain$' { $retain = 'true'; continue }
        '^--retain=(.+)$' { $retain = $Matches[1]; continue }
        '^(-h|--help|help)$' {
            @'
concord manager

Usage:
  manage.ps1 install [--channel stable|beta] [--version X.Y.Z] [--retain[=true|false]]
  manage.ps1 update [--channel stable|beta] [--version X.Y.Z] [--retain[=true|false]]
  manage.ps1 uninstall [--version X.Y.Z]

Options:
  --public-url <url>     release metadata and artifact base URL
  --install-root <path>  versioned install root
  --bin-dir <path>       directory for the concord executable

Environment:
  CONCORD_RELEASES_PUBLIC_URL
  CONCORD_CHANNEL
  CONCORD_VERSION
  CONCORD_INSTALL_ROOT
  CONCORD_LOCAL_BIN_DIR
  CONCORD_RETAIN
'@ | Write-Output
            exit 0
        }
        default { throw "unknown argument: $arg" }
    }
}

if ($channel -notin @('stable', 'beta')) {
    throw "invalid channel: $channel"
}
if ($retain -notin @('true', 'false')) {
    throw "invalid --retain value: $retain"
}
$publicUrl = $publicUrl.TrimEnd('/')

function Normalize-Version {
    param([string]$Value)
    $held = $Value.TrimStart('v')
    if ($held -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(-beta\.[1-9][0-9]*)?$') {
        throw "invalid concord version: $Value"
    }
    return $held
}

function Root-Owned {
    $path = Join-Path $installRoot '.concord-manager'
    return [System.IO.File]::Exists($path) -and ((Get-Content -LiteralPath $path -TotalCount 1) -eq $marker)
}

function Version-Owned {
    param([string]$Seat)
    $path = Join-Path (Join-Path $installRoot $Seat) '.concord-manager'
    if (![System.IO.File]::Exists($path)) {
        return $false
    }
    $lines = @(Get-Content -LiteralPath $path)
    return $lines.Length -ge 2 -and $lines[0] -eq $marker -and $lines[1] -eq "version=$Seat"
}

function Ensure-Root {
    if ([System.IO.Directory]::Exists($installRoot)) {
        if (!(Root-Owned)) {
            throw "refusing unowned install root: $installRoot"
        }
        return
    }
    if ([System.IO.File]::Exists($installRoot)) {
        throw "install root is not a directory: $installRoot"
    }
    New-Item -ItemType Directory -Force -Path $installRoot | Out-Null
    Set-Content -LiteralPath (Join-Path $installRoot '.concord-manager') -Value $marker
}

function File-Hash {
    param([string]$Path)
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Entry-Owned {
    param([string]$Entry)
    if (![System.IO.File]::Exists($Entry) -or !(Root-Owned)) {
        return $false
    }
    $entryHash = File-Hash $Entry
    foreach ($found in @(Get-ChildItem -LiteralPath $installRoot -Directory -ErrorAction SilentlyContinue)) {
        if (!(Version-Owned $found.Name)) {
            continue
        }
        $candidate = Join-Path $found.FullName 'concord.exe'
        if ([System.IO.File]::Exists($candidate) -and (File-Hash $candidate) -eq $entryHash) {
            return $true
        }
    }
    return $false
}

function Install-Concord {
    $resolvedVersion = $version
    if ([string]::IsNullOrWhiteSpace($resolvedVersion)) {
        $latest = Invoke-RestMethod -Uri "$publicUrl/$channel/latest/metadata.json"
        $resolvedVersion = $latest.releaseVersion
        if ([string]::IsNullOrWhiteSpace($resolvedVersion)) {
            throw 'failed to resolve latest concord version'
        }
    }
    $resolvedVersion = Normalize-Version $resolvedVersion
    $seatVersion = "v$resolvedVersion"
    $archive = 'concord-x86_64-pc-windows-msvc.zip'
    $name = 'concord-x86_64-pc-windows-msvc'
    $tmpdir = Join-Path ([System.IO.Path]::GetTempPath()) ("concord-" + [System.Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmpdir | Out-Null
    try {
        $metadata = Invoke-RestMethod -Uri "$publicUrl/$channel/versions/$resolvedVersion/metadata.json"
        if ((Normalize-Version $metadata.releaseVersion) -ne $resolvedVersion) {
            throw "metadata version mismatch: expected $resolvedVersion got $($metadata.releaseVersion)"
        }
        $asset = @($metadata.assets | Where-Object { $_.name -eq $archive })
        if ($asset.Length -ne 1 -or [string]::IsNullOrWhiteSpace($asset[0].sha256)) {
            throw "metadata missing sha256 for $archive"
        }
        $archivePath = Join-Path $tmpdir $archive
        Invoke-WebRequest -Uri "$publicUrl/$channel/versions/$resolvedVersion/$archive" -OutFile $archivePath
        $actual = File-Hash $archivePath
        if ($actual -ne $asset[0].sha256.ToLowerInvariant()) {
            throw "checksum mismatch for ${archive}: expected $($asset[0].sha256) got $actual"
        }
        Expand-Archive -LiteralPath $archivePath -DestinationPath $tmpdir -Force
        $candidate = Join-Path (Join-Path $tmpdir $name) 'concord.exe'
        if (![System.IO.File]::Exists($candidate)) {
            throw 'archive missing concord.exe'
        }
        $stagedVersion = (& $candidate --version | Out-String).Trim()
        if ($stagedVersion -notmatch [regex]::Escape($resolvedVersion)) {
            throw "binary version mismatch: $stagedVersion"
        }

        $entry = Join-Path $localBinDir 'concord.exe'
        if ([System.IO.File]::Exists($entry) -and !(Entry-Owned $entry) -and (File-Hash $entry) -ne (File-Hash $candidate)) {
            throw "refusing unowned concord binary: $entry; install its exact version first"
        }

        Ensure-Root
        $target = Join-Path $installRoot $seatVersion
        if ([System.IO.Directory]::Exists($target)) {
            if (!(Version-Owned $seatVersion)) {
                throw "refusing unowned version seat: $target"
            }
            if ((File-Hash (Join-Path $target 'concord.exe')) -ne (File-Hash $candidate)) {
                throw "refusing mismatched version seat: $target"
            }
        }
        else {
            $stage = Join-Path $installRoot (".concord-stage-" + [System.Guid]::NewGuid().ToString('N'))
            New-Item -ItemType Directory -Path $stage | Out-Null
            Copy-Item -LiteralPath $candidate -Destination (Join-Path $stage 'concord.exe')
            Set-Content -LiteralPath (Join-Path $stage '.concord-manager') -Value @($marker, "version=$seatVersion")
            Move-Item -LiteralPath $stage -Destination $target
        }

        New-Item -ItemType Directory -Force -Path $localBinDir | Out-Null
        $next = Join-Path $localBinDir (".concord-" + [System.Guid]::NewGuid().ToString('N') + '.exe')
        Copy-Item -LiteralPath (Join-Path $target 'concord.exe') -Destination $next
        Move-Item -Force -LiteralPath $next -Destination $entry
        & $entry --version

        if ($retain -eq 'false') {
            foreach ($found in @(Get-ChildItem -LiteralPath $installRoot -Directory)) {
                if ($found.Name -eq $seatVersion -or !(Version-Owned $found.Name)) {
                    continue
                }
                Remove-Item -Recurse -Force -LiteralPath $found.FullName
                Write-Output "removed old concord $($found.Name) from $installRoot"
            }
        }
        Write-Output "installed concord $seatVersion to $entry"
    }
    finally {
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $tmpdir
    }
}

function Uninstall-Concord {
    $entry = Join-Path $localBinDir 'concord.exe'
    if (![string]::IsNullOrWhiteSpace($version)) {
        $seatVersion = "v$(Normalize-Version $version)"
        $target = Join-Path $installRoot $seatVersion
        if ([System.IO.Directory]::Exists($target)) {
            if (!(Root-Owned) -or !(Version-Owned $seatVersion)) {
                throw "refusing unowned version seat: $target"
            }
            if ([System.IO.File]::Exists($entry) -and (File-Hash $entry) -eq (File-Hash (Join-Path $target 'concord.exe'))) {
                Remove-Item -Force -LiteralPath $entry
            }
            Remove-Item -Recurse -Force -LiteralPath $target
        }
        Write-Output "removed concord $seatVersion from $installRoot"
        return
    }

    if ([System.IO.Directory]::Exists($installRoot) -and !(Root-Owned)) {
        throw "refusing unowned install root: $installRoot"
    }
    if ([System.IO.File]::Exists($entry)) {
        if (!(Entry-Owned $entry)) {
            throw "refusing unowned concord binary: $entry"
        }
        Remove-Item -Force -LiteralPath $entry
    }
    if ([System.IO.Directory]::Exists($installRoot)) {
        foreach ($found in @(Get-ChildItem -LiteralPath $installRoot -Directory)) {
            if (Version-Owned $found.Name) {
                Remove-Item -Recurse -Force -LiteralPath $found.FullName
            }
        }
        $held = @(Get-ChildItem -Force -LiteralPath $installRoot | Where-Object { $_.Name -ne '.concord-manager' })
        if ($held.Length -eq 0) {
            Remove-Item -Force -LiteralPath (Join-Path $installRoot '.concord-manager')
            Remove-Item -Force -LiteralPath $installRoot
        }
        else {
            [Console]::Error.WriteLine("preserved unowned content under $installRoot")
        }
    }
    Write-Output "removed concord from $installRoot and $entry"
}

switch ($command) {
    { $_ -in @('install', 'update') } { Install-Concord }
    'uninstall' { Uninstall-Concord }
    default { throw "unknown command: $command" }
}
