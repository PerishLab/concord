param(
    [string]$Root,
    [ValidateSet('survey', 'stage', 'resume', 'activate')]
    [string]$Action,
    [string]$Fingerprint,
    [switch]$Apply,
    [switch]$Help
)

$ErrorActionPreference = 'Stop'
$managerUrl = 'https://releases.concord.perish.uk/v1/objects/sha256/bd8eb9515100feeb22550b3176b09831b1c7f4881059d78bacbd5ceabd5c05b6/manage.ps1'
$managerSha = 'BD8EB9515100FEB22550B3176B09831B1C7F4881059D78BACBD5CEABD5C05B6'

function Show-Help {
    @'
Concord v0.11.0 legacy transition

Usage:
  migration.ps1 -Root SPACE -Action survey
  migration.ps1 -Root SPACE -Action stage
  migration.ps1 -Root SPACE -Action resume -Fingerprint FINGERPRINT
  migration.ps1 -Root SPACE -Action activate -Fingerprint FINGERPRINT -Apply

The script validates the exact migration territory, obtains the immutable
v0.10.0 engine in an isolated temporary seat, and never changes the installed
Concord binary. Activation remains an explicit destructive authority.
'@ | Write-Output
}

if ($Help) {
    Show-Help
    exit 0
}
if ([string]::IsNullOrWhiteSpace($Root)) {
    throw '-Root SPACE is required'
}
if ([string]::IsNullOrWhiteSpace($Action)) {
    throw '-Action is required'
}
if ($Action -in @('resume', 'activate') -and [string]::IsNullOrWhiteSpace($Fingerprint)) {
    throw "$Action requires -Fingerprint"
}
if ($Action -notin @('resume', 'activate') -and ![string]::IsNullOrWhiteSpace($Fingerprint)) {
    throw "$Action does not accept -Fingerprint"
}
if ($Action -eq 'activate' -and !$Apply) {
    throw 'activate requires -Apply'
}
if ($Action -ne 'activate' -and $Apply) {
    throw "$Action does not accept -Apply"
}

$spaceItem = Get-Item -LiteralPath $Root -Force
if (!$spaceItem.PSIsContainer -or ($spaceItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'Space root must be one real directory'
}
$space = $spaceItem.FullName
$estate = Join-Path $space '.concord'
$migration = Join-Path $estate 'migration'
$release = Join-Path $migration 'v0.10.0'
$stage = Join-Path $release 'stage'
$rollback = Join-Path $release 'rollback'

function Assert-Directory {
    param([string]$Path)
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    if ($null -eq $item) {
        return
    }
    if (!$item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "managed directory is not direct: $Path"
    }
}

function Assert-File {
    param([string]$Path)
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    if ($null -eq $item) {
        return
    }
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "managed file is not regular: $Path"
    }
}

function Assert-Entries {
    param([string]$Path, [string[]]$Names)
    if (!(Test-Path -LiteralPath $Path)) {
        return
    }
    foreach ($item in Get-ChildItem -LiteralPath $Path -Force) {
        if ($item.Name -notin $Names) {
            throw "unknown migration territory: $($item.FullName)"
        }
    }
}

function Assert-Stage {
    Assert-Directory $stage
    Assert-Entries $stage @('estate.sqlite3', 'sudo')
    Assert-File (Join-Path $stage 'estate.sqlite3')
    Assert-File (Join-Path $stage 'sudo')
}

function Assert-Before {
    Assert-Directory $estate
    $database = Join-Path $estate 'estate.sqlite3'
    $sudo = Join-Path $estate 'sudo'
    Assert-File $database
    Assert-File $sudo
    if ((Test-Path -LiteralPath $database) -or (Test-Path -LiteralPath $sudo)) {
        throw 'the Space already carries an active or partial estate'
    }
    Assert-Entries $estate @('migration')
    Assert-Directory $migration
    Assert-Entries $migration @('v0.10.0')
    Assert-Directory $release
    Assert-Entries $release @('stage')
    Assert-Stage
}

function Assert-After {
    Assert-Directory $estate
    Assert-Directory $migration
    Assert-Directory $release
    if ($Action -eq 'activate') {
        Assert-Entries $estate @('estate.sqlite3', 'migration', 'sudo')
        Assert-Entries $migration @('v0.10.0')
        Assert-Entries $release @('rollback')
        Assert-File (Join-Path $estate 'estate.sqlite3')
        Assert-File (Join-Path $estate 'sudo')
        Assert-Directory $rollback
    }
    else {
        Assert-Entries $estate @('migration')
        Assert-Entries $migration @('v0.10.0')
        Assert-Entries $release @('stage')
        Assert-Stage
    }
}

Assert-Before
$temporary = Join-Path ([IO.Path]::GetTempPath()) "concord-v0.10.0-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    $manager = Join-Path $temporary 'manage.ps1'
    Invoke-WebRequest -UseBasicParsing -Uri $managerUrl -OutFile $manager
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $manager).Hash
    if ($actual -cne $managerSha) {
        throw 'v0.10.0 manager digest mismatch'
    }
    $install = Join-Path $temporary 'install'
    $bin = Join-Path $temporary 'bin'
    $managerOutput = & $manager install --channel stable --version v0.10.0 --install-root $install --bin-dir $bin
    if ($LASTEXITCODE -ne 0) {
        throw 'v0.10.0 manager failed'
    }
    foreach ($line in $managerOutput) {
        [Console]::Error.WriteLine($line)
    }
    $engine = Join-Path $bin 'concord.exe'
    if (!(Test-Path -LiteralPath $engine -PathType Leaf)) {
        throw 'v0.10.0 manager produced no engine'
    }
    $arguments = @('--root', $space, '--json', 'migration', $Action)
    if ($Action -in @('resume', 'activate')) {
        $arguments += $Fingerprint
    }
    if ($Action -eq 'activate') {
        $arguments += '--apply'
    }
    & $engine @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "v0.10.0 migration engine failed with exit code $LASTEXITCODE"
    }
    Assert-After
}
finally {
    Remove-Item -LiteralPath $temporary -Recurse -Force -ErrorAction SilentlyContinue
}
