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

New-Item -ItemType Directory -Force $Root | Out-Null
try {
  Invoke-WebRequest "$Base/$Channel/versions/$Version/$Name.zip" -OutFile $Archive
  Expand-Archive $Archive -DestinationPath $Root
  & (Join-Path $Root "$Name/concord.exe") --version
  if ($LASTEXITCODE -ne 0) {
    throw "concord smoke failed: $LASTEXITCODE"
  }
} finally {
  Remove-Item -Recurse -Force $Root -ErrorAction SilentlyContinue
}
