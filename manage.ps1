param(
  [ValidateSet("install", "uninstall")]
  [string]$Command = "install",
  [string]$Version = ""
)

$ErrorActionPreference = "Stop"
$Base = if ($env:CONCORD_RELEASES_PUBLIC_URL) {
  $env:CONCORD_RELEASES_PUBLIC_URL.TrimEnd("/")
} else {
  "https://releases.concord.perish.uk"
}
$Seat = if ($env:CONCORD_INSTALL_DIR) {
  $env:CONCORD_INSTALL_DIR
} else {
  Join-Path $HOME ".local/bin"
}
$Binary = Join-Path $Seat "concord.exe"

if ($Command -eq "uninstall") {
  Remove-Item $Binary -ErrorAction SilentlyContinue
  exit
}
if ([string]::IsNullOrWhiteSpace($Version)) {
  $Version = (Invoke-RestMethod "$Base/stable/latest/metadata.json").version
}
$Name = "concord-x86_64-pc-windows-msvc"
$Root = Join-Path ([System.IO.Path]::GetTempPath()) "concord-install-$PID"
New-Item -ItemType Directory -Force $Root | Out-Null
try {
  $Archive = Join-Path $Root "$Name.zip"
  Invoke-WebRequest "$Base/stable/versions/$Version/$Name.zip" -OutFile $Archive
  Expand-Archive $Archive -DestinationPath $Root
  New-Item -ItemType Directory -Force $Seat | Out-Null
  Copy-Item (Join-Path $Root "$Name/concord.exe") $Binary -Force
  & $Binary --version
} finally {
  Remove-Item -Recurse -Force $Root -ErrorAction SilentlyContinue
}
