# Resonance MSIX packaging.
#
# Builds the app (release), stages the package layout (exe + manifest +
# tile assets) and packs it into Resonance.msix with makeappx.exe.
# The package comes out UNSIGNED: the Microsoft Store signs submissions
# itself, so an unsigned .msix is exactly what Partner Center wants.
# To install it on your own machine for a local test you must sign it
# first - see STORE.md, "Testing the package locally".
#
# Run from anywhere (paths resolve from this script's location):
#   .\crates\resonance\packaging\package.ps1
#   .\crates\resonance\packaging\package.ps1 -Version 1.1.0.0

param(
    [string]$Version = "1.0.0.0",
    [string]$OutDir = "$PSScriptRoot\build"
)

$ErrorActionPreference = "Stop"

$crateDir = Split-Path $PSScriptRoot -Parent          # crates\resonance
$repoRoot = Split-Path (Split-Path $crateDir -Parent) -Parent

# 1. Release build (MSVC target - the one with a working SQLite build).
Write-Host "Building resonance (release)..."
Push-Location $repoRoot
try {
    cargo build --release -p resonance
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
} finally {
    Pop-Location
}
$exe = Join-Path $repoRoot "target\release\resonance.exe"
if (-not (Test-Path $exe)) { throw "Built exe not found at $exe" }

# 2. Stage the layout (tile assets are generated, not committed).
$assetNames = @("Square44x44Logo.png", "StoreLogo.png", "Square150x150Logo.png",
    "Wide310x150Logo.png", "Square310x310Logo.png")
$missing = $assetNames | Where-Object { -not (Test-Path (Join-Path $PSScriptRoot "assets\$_")) }
if ($missing) {
    Write-Host "Generating tile assets..."
    & "$PSScriptRoot\make-assets.ps1"
}
$layout = Join-Path $OutDir "layout"
if (Test-Path $layout) { Remove-Item $layout -Recurse -Force }
New-Item -ItemType Directory -Force -Path (Join-Path $layout "assets") | Out-Null
Copy-Item $exe (Join-Path $layout "resonance.exe")
Copy-Item "$PSScriptRoot\assets\*.png" (Join-Path $layout "assets")

# The staged manifest gets the requested version stamped in.
$manifest = Get-Content "$PSScriptRoot\Package.appxmanifest" -Raw
if ($manifest -match "REPLACE-") {
    Write-Warning "The manifest still has REPLACE-* identity values. Fill them from Partner Center (Product identity) before uploading - see STORE.md. Packing anyway for local testing."
}
$manifest = $manifest -replace 'Version="[0-9.]+"', "Version=`"$Version`""
Set-Content (Join-Path $layout "AppxManifest.xml") $manifest -Encoding UTF8

# 3. Find makeappx.exe (Windows SDK) and pack.
$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending | Select-Object -First 1
if (-not $makeappx) {
    throw "makeappx.exe not found. Install the Windows 10/11 SDK (it is part of Visual Studio's 'Desktop development with C++' / Windows SDK workloads)."
}
$msix = Join-Path $OutDir "Resonance.msix"
if (Test-Path $msix) { Remove-Item $msix -Force }
& $makeappx.FullName pack /d $layout /p $msix /o
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }

Write-Host ""
Write-Host "Packed: $msix"
Write-Host "Next: upload it in Partner Center (Resonance > Submission > Packages), or sign it for a local test - see STORE.md."
