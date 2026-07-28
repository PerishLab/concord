param(
  [Parameter(Mandatory = $true)]
  [string]$Version,
  [Parameter(Mandatory = $true)]
  [string]$Channel
)

$ErrorActionPreference = "Stop"
$Base = $env:CONCORD_RELEASES_PUBLIC_URL.TrimEnd("/")
$Name = "concord-x86_64-pc-windows-msvc"
$Root = Join-Path ([System.IO.Path]::GetTempPath()) "concord-smoke-$PID"
$Archive = Join-Path $Root "$Name.zip"
$Manager = Join-Path $Root "manage.ps1"

New-Item -ItemType Directory -Force $Root | Out-Null
try {
  Invoke-WebRequest "$Base/$Channel/versions/$Version/$Name.zip" -OutFile $Archive
  Invoke-WebRequest "$Base/manage.ps1" -OutFile $Manager
  Expand-Archive $Archive -DestinationPath $Root
  $Released = Join-Path $Root "$Name/concord.exe"
  & $Released --version
  if ($LASTEXITCODE -ne 0) {
    throw "concord smoke failed: $LASTEXITCODE"
  }

  $Install = Join-Path $Root "install"
  $BinDir = Join-Path $Root "bin"
  $Bin = Join-Path $BinDir "concord.exe"
  & $Manager install --channel $Channel --version $Version --public-url $Base `
    --install-root $Install --bin-dir $BinDir
  & $Bin --version
  if ($LASTEXITCODE -ne 0) {
    throw "installed concord failed: $LASTEXITCODE"
  }
  & $Manager update --channel $Channel --version $Version --public-url $Base `
    --install-root $Install --bin-dir $BinDir
  & $Manager uninstall --version $Version --install-root $Install --bin-dir $BinDir
  if (Test-Path (Join-Path $Install "v$Version")) {
    throw "version uninstall left v$Version"
  }
  & $Manager uninstall --install-root $Install --bin-dir $BinDir
  if (Test-Path $Install) {
    throw "full uninstall left $Install"
  }

  $LegacyInstall = Join-Path $Root "legacy-install"
  $LegacyBinDir = Join-Path $Root "legacy-bin"
  New-Item -ItemType Directory -Force $LegacyBinDir | Out-Null
  Copy-Item $Released (Join-Path $LegacyBinDir "concord.exe")
  & $Manager install --channel $Channel --version $Version --public-url $Base `
    --install-root $LegacyInstall --bin-dir $LegacyBinDir
  & $Manager uninstall --install-root $LegacyInstall --bin-dir $LegacyBinDir

  $RefusalInstall = Join-Path $Root "refusal-install"
  $RefusalBinDir = Join-Path $Root "refusal-bin"
  New-Item -ItemType Directory -Force $RefusalBinDir | Out-Null
  Set-Content (Join-Path $RefusalBinDir "concord.exe") "unowned"
  $Refused = $false
  try {
    & $Manager install --channel $Channel --version $Version --public-url $Base `
      --install-root $RefusalInstall --bin-dir $RefusalBinDir
  }
  catch {
    $Refused = $true
  }
  if (!$Refused) {
    throw "manager accepted an unowned entrypoint"
  }
  if (Test-Path $RefusalInstall) {
    throw "refused install created $RefusalInstall"
  }
} finally {
  Remove-Item -Recurse -Force $Root -ErrorAction SilentlyContinue
}
