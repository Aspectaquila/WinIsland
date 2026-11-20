# Builds dist\WinIslandSetup.exe: publishes the app self-contained (no .NET install needed on the target PC),
# packs it with the web UI into payload.zip, and embeds that in the .NET Framework 4.8 setup wizard.
param([switch]$SkipApp)
$ErrorActionPreference = 'Stop'
$root  = Split-Path $PSScriptRoot -Parent
$stage = Join-Path $PSScriptRoot 'obj\stage'
$setup = Join-Path $PSScriptRoot 'Setup'
$dist  = Join-Path $root 'dist'

if (-not $SkipApp) {
  Write-Host '1/4 Publishing WinIsland (self-contained, single file)...'
  if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
  $pub = Join-Path $stage 'payload\bin'
  # OutDir is redirected so the publish never touches the developer build in ..\bin
  dotnet publish "$root\src\Frost.csproj" -c Release -r win-x64 --self-contained true -nologo -v q `
    -p:PublishSingleFile=true -p:EnableCompressionInSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true `
    -p:DebugType=none -p:OutDir="$stage\appbuild\\" -o $pub
  if ($LASTEXITCODE) { throw 'dotnet publish failed' }
  Get-ChildItem $pub -Include *.pdb, *.xml -Recurse | Remove-Item
  if (-not (Test-Path "$pub\frost.ico")) { Copy-Item "$root\src\frost.ico" $pub }

  Write-Host '2/4 Adding the web UI...'
  Copy-Item "$root\web" (Join-Path $stage 'payload\web') -Recurse

  Write-Host '3/4 Packing payload.zip...'
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

Write-Host '4/4 Building the setup wizard...'
Copy-Item "$root\web\icon.svg" "$setup\ui\icon.svg" -Force
dotnet build "$setup\Setup.csproj" -c Release -nologo -v q
if ($LASTEXITCODE) { throw 'setup build failed' }
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item "$setup\bin\WinIslandSetup.exe" "$dist\WinIslandSetup.exe" -Force
$mb = [math]::Round((Get-Item "$dist\WinIslandSetup.exe").Length / 1MB, 1)
Write-Host "Done: $dist\WinIslandSetup.exe ($mb MB)"
