# Builds dist\WinIslandSetup.exe: compiles the Rust/GPUI app (winisland\), packs it into payload.zip,
# and embeds that in the .NET Framework 4.8 setup wizard.
# The app is one native exe: no .NET runtime and no WebView2 needed on the target PC.
param([switch]$SkipApp)
$ErrorActionPreference = 'Stop'
$root  = Split-Path $PSScriptRoot -Parent
$stage = Join-Path $PSScriptRoot 'obj\stage'
$setup = Join-Path $PSScriptRoot 'Setup'
$dist  = Join-Path $root 'dist'

if (-not $SkipApp) {
  Write-Host '1/3 Building WinIsland (Rust + GPUI, release)...'
  if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
  $pub = Join-Path $stage 'payload\bin'
  New-Item -ItemType Directory -Force $pub | Out-Null
  # rustc occasionally crashes on this machine (STATUS_ACCESS_VIOLATION); a retry always works
  $ok = $false
  for ($i = 0; $i -lt 3 -and -not $ok; $i++) {
    # cargo reports progress on stderr, which Windows PowerShell would turn into a terminating error
    cmd /c "cargo build --release --manifest-path `"$root\winisland\Cargo.toml`" 2>&1" | Out-Host
    $ok = ($LASTEXITCODE -eq 0)
  }
  if (-not $ok) { throw 'cargo build failed' }
  Copy-Item "$root\winisland\target\release\winisland.exe" "$pub\WinIsland.exe"

  Write-Host '2/3 Packing payload.zip...'
  Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
  $zip = Join-Path $setup 'payload.zip'
  if (Test-Path $zip) { Remove-Item $zip }
  # entries are written by hand so their names use '/' as the zip spec requires
  $src = Join-Path $stage 'payload'
  $fs = [IO.File]::Open($zip, 'Create')
  $za = New-Object IO.Compression.ZipArchive($fs, [IO.Compression.ZipArchiveMode]::Create)
  foreach ($f in Get-ChildItem $src -Recurse -File) {
    $name = $f.FullName.Substring($src.Length + 1).Replace('\', '/')
    $e = $za.CreateEntry($name, [IO.Compression.CompressionLevel]::Optimal)
    $e.LastWriteTime = $f.LastWriteTime
    $out = $e.Open(); $in = [IO.File]::OpenRead($f.FullName); $in.CopyTo($out); $in.Dispose(); $out.Dispose()
  }
  $za.Dispose(); $fs.Dispose()
}

Write-Host '3/3 Building the setup wizard...'
Copy-Item "$root\web\icon.svg" "$setup\ui\icon.svg" -Force
dotnet build "$setup\Setup.csproj" -c Release -nologo -v q
if ($LASTEXITCODE) { throw 'setup build failed' }
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item "$setup\bin\WinIslandSetup.exe" "$dist\WinIslandSetup.exe" -Force
$mb = [math]::Round((Get-Item "$dist\WinIslandSetup.exe").Length / 1MB, 1)
Write-Host "Done: $dist\WinIslandSetup.exe ($mb MB)"