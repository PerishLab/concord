param(
  [Parameter(Mandatory = $true)]
  [string]$Version
)

$ErrorActionPreference = "Stop"
$Target = "x86_64-pc-windows-msvc"
$Name = "concord-$Target"
$Root = (git rev-parse --show-toplevel).Trim()
$Stage = Join-Path $Root "dist/stage/$Name"

if ([string]::IsNullOrWhiteSpace($Version)) {
  throw "release version is empty"
}
if (Test-Path $Stage) {
  Remove-Item -Recurse -Force $Stage
}
New-Item -ItemType Directory -Force $Stage | Out-Null
cargo build --locked --release --target $Target -p concord
Copy-Item (Join-Path $Root "target/$Target/release/concord.exe") $Stage
Copy-Item (Join-Path $Root "README.md") $Stage
Copy-Item (Join-Path $Root "LICENSE") $Stage
New-Item -ItemType Directory -Force (Join-Path $Root "dist") | Out-Null
Compress-Archive -Path $Stage -DestinationPath (Join-Path $Root "dist/$Name.zip") -Force
