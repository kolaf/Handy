<#
.SYNOPSIS  Switch Handy's language / post-processing prompt / model from the command line.
.DESCRIPTION
  Handy keeps its settings in memory and has no remote control for them, so this script
  closes Handy, edits settings_store.json, and starts Handy again (hidden in the tray).
  Profiles are defined in profiles.json next to this script. Each profile may set:
    language (e.g. "no", "en", "auto"), prompt (post-processing prompt id), model (selected_model id).
.EXAMPLE   .\handy-profile.ps1 no-formal
.EXAMPLE   .\handy-profile.ps1 en-simple -DryRun
.EXAMPLE   .\handy-profile.ps1 -List
#>
param(
  [Parameter(Position = 0)][string]$Profile,
  [switch]$List,
  [switch]$DryRun
)
$ErrorActionPreference = 'Stop'
$profiles = Get-Content (Join-Path $PSScriptRoot 'profiles.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ($List -or -not $Profile) { $profiles.PSObject.Properties | ForEach-Object { '{0,-14} {1}' -f $_.Name, ($_.Value | ConvertTo-Json -Compress) }; return }
$p = $profiles.$Profile
if (-not $p) { throw "Unknown profile '$Profile'. Use -List." }

$store = Join-Path $env:APPDATA 'com.pais.handy\settings_store.json'
$exe   = Join-Path $env:LOCALAPPDATA 'Handy\handy.exe'
if (-not (Test-Path $store)) { throw "Settings file not found: $store" }
if (-not (Test-Path $exe))   { throw "handy.exe not found: $exe" }

$raw = [IO.File]::ReadAllText($store, [Text.UTF8Encoding]::new($false))
$doc = $raw | ConvertFrom-Json            # read-only: used for validation, never written back
$s = if ($doc.PSObject.Properties['settings']) { $doc.settings } else { $doc }
if ($p.prompt -and -not ($s.post_process_prompts | Where-Object { $_.id -eq $p.prompt })) {
  throw "Prompt id '$($p.prompt)' is not in Handy's prompt list."
}

function Set-JsonString([string]$text, [string]$key, [string]$value) {
  # Targeted replace so the rest of the file stays byte-for-byte untouched.
  $pattern = '("' + [regex]::Escape($key) + '"\s*:\s*)(?:"[^"]*"|null)'
  $n = [regex]::Matches($text, $pattern).Count
  if ($n -ne 1) { throw "Expected exactly one '$key' in settings, found $n. Not touching the file." }
  [regex]::Replace($text, $pattern, { param($m) $m.Groups[1].Value + (ConvertTo-Json $value -Compress) })
}

$new = $raw
if ($p.language) { $new = Set-JsonString $new 'selected_language' $p.language }
if ($p.prompt)   { $new = Set-JsonString $new 'post_process_selected_prompt_id' $p.prompt }
if ($p.model)    { $new = Set-JsonString $new 'selected_model' $p.model }
$null = $new | ConvertFrom-Json            # must still be valid JSON

Write-Host "Profile '$Profile': language=$($p.language) prompt=$($p.prompt) model=$($p.model)"
if ($DryRun) { Write-Host '(dry run - nothing changed)'; return }

$running = Get-Process -Name handy -ErrorAction SilentlyContinue
if ($running) { $running | Stop-Process; $running | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue }
Copy-Item $store "$store.profile-backup" -Force
[IO.File]::WriteAllText($store, $new, [Text.UTF8Encoding]::new($false))
Start-Process -FilePath $exe -ArgumentList '--start-hidden'
Write-Host 'Handy restarted.'
