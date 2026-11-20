<#
  WinIsland: clean uninstall.
    powershell -ExecutionPolicy Bypass -File "$env:USERPROFILE\FrostDesktop\Uninstall.ps1"              # stop + remove autostart, keep files
    powershell -ExecutionPolicy Bypass -File "$env:USERPROFILE\FrostDesktop\Uninstall.ps1" -RemoveFiles # ...and delete FrostDesktop

  WinIsland only ever adds one autostart entry and one Start-menu shortcut; this removes both.
#>
param([switch]$RemoveFiles)
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$exe = Join-Path $root 'bin\WinIsland.exe'

if (Test-Path $exe) {
    & $exe --restore   # running instance: turn off autostart and exit
    for ($i = 0; $i -lt 40 -and (Get-Process Frost -ErrorAction SilentlyContinue); $i++) { Start-Sleep -Milliseconds 250 }
    if (Get-Process Frost -ErrorAction SilentlyContinue) { Get-Process Frost | Stop-Process -Force }
}
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'FrostDesktop' -ErrorAction SilentlyContinue
foreach ($n in 'WinIsland Settings.lnk', 'Frost Control Panel.lnk') {
    $lnk = Join-Path ([Environment]::GetFolderPath('Programs')) $n
    if (Test-Path $lnk) { Remove-Item $lnk -Force }
}

if ($RemoveFiles) {
    Start-Sleep -Seconds 2   # let WebView2 processes exit
    Set-Location $env:USERPROFILE
    Remove-Item -Recurse -Force $root
    Write-Host 'WinIsland has been removed completely.' -ForegroundColor Green
} else {
    Write-Host "WinIsland stopped and removed from startup. Files remain in $root (re-run with -RemoveFiles to delete them)." -ForegroundColor Green
}
