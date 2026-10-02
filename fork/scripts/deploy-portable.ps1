<#
.SYNOPSIS  Update a portable Handy folder with the latest build without touching its data.
.DESCRIPTION
  Copies the files staged by build-portable.ps1 (C:\dev\Handy-dev-portable by default) into -Target,
  leaving Target\Data (settings, models, history, recordings) alone. Refuses to run while that copy
  of Handy is running, because Windows cannot replace a running exe.

  -Restart: if that copy is running, stop it (any dictation in progress is lost), deploy, and start it
  again hidden in the tray. Without -Restart the script refuses to run while the copy is open.

  Tip: run Handy from this deployed folder, not from the build output or the staging folder. Builds then
  never collide with a running Handy, and you only stop it for the moment of the deploy.

  First time only: -SeedFromInstalled copies settings_store.json and the models folder from the normal
  (non-portable) profile in %APPDATA%\com.pais.handy into Target\Data, so your endpoint, key, prompts
  and downloaded models carry over. It never overwrites an existing Data folder.
.EXAMPLE   .\deploy-portable.ps1 -Target D:\Handy -SeedFromInstalled
.EXAMPLE   .\deploy-portable.ps1 -Target D:\Handy
#>
param(
  [Parameter(Mandatory = $true)][string]$Target,
  [string]$Source = 'C:\dev\Handy-dev-portable',
  [switch]$SeedFromInstalled,
  [switch]$Restart
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path (Join-Path $Source 'handy.exe'))) { throw "No build staged in $Source. Run build-portable.ps1 first." }

$targetExe = Join-Path $Target 'handy.exe'
$fullTarget = [IO.Path]::GetFullPath($targetExe)
$running = Get-Process -Name handy -ErrorAction SilentlyContinue | Where-Object { $_.Path -and [string]::Equals($_.Path, $fullTarget, [StringComparison]::OrdinalIgnoreCase) }
$wasRunning = [bool]$running
if ($running) {
  if (-not $Restart) { throw "Handy is running from $Target. Close it first, or use -Restart." }
  $running | Stop-Process -Force
  $running | Wait-Process -Timeout 15 -ErrorAction SilentlyContinue
}

New-Item -ItemType Directory -Force -Path $Target | Out-Null
# /E subfolders, /XD Data never touches the data folder, /NFL /NDL /NJH quiet output
robocopy $Source $Target /E /XD Data /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) { throw "robocopy failed with exit code $LASTEXITCODE" }

$marker = Join-Path $Target 'portable'
if (-not (Test-Path $marker)) { Set-Content -Path $marker -Value 'Handy Portable Mode' -NoNewline }

$data = Join-Path $Target 'Data'
if ($SeedFromInstalled) {
  if (Test-Path $data) {
    Write-Host "Data already exists in $Target - not seeding (it would overwrite your settings)."
  } else {
    $installed = Join-Path $env:APPDATA 'com.pais.handy'
    New-Item -ItemType Directory -Force -Path $data | Out-Null
    if (Test-Path (Join-Path $installed 'settings_store.json')) {
      Copy-Item (Join-Path $installed 'settings_store.json') $data
      Write-Host 'Seeded settings_store.json'
    }
    if (Test-Path (Join-Path $installed 'models')) {
      Copy-Item (Join-Path $installed 'models') (Join-Path $data 'models') -Recurse
      Write-Host 'Seeded models'
    }
  }
}
$exe = Get-Item $targetExe
if ($Restart -and $wasRunning) {
  Start-Process -FilePath $targetExe -ArgumentList '--start-hidden'
  Write-Host 'Restarted.'
}
Write-Host "Deployed to $Target  (handy.exe $($exe.LastWriteTime))  Data preserved: $(Test-Path $data)"
