$ErrorActionPreference = 'Continue'
$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
$env:VULKAN_SDK = [Environment]::GetEnvironmentVariable('VULKAN_SDK','Machine')
Set-Location C:\dev\Handy
$sha = (git rev-parse --short HEAD).Trim()
Set-Location src-tauri
cargo test --release --lib 2>&1 | Select-String "test result|FAILED|panicked|^error|error\["
"TEST EXIT: $LASTEXITCODE"
if ($LASTEXITCODE -ne 0) { "TESTS FAILED - not building"; "PORTABLE DONE (FAILED)"; exit 1 }
Set-Location C:\dev\Handy
bun run tauri build --no-bundle
"BUILD EXIT: $LASTEXITCODE"
if ($LASTEXITCODE -ne 0) { "BUILD FAILED - not packaging"; "PORTABLE DONE (FAILED)"; exit 1 }
$rel = 'C:\dev\Handy\src-tauri\target\release'; $out = 'C:\dev\Handy-dev-portable'
Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue; New-Item -ItemType Directory $out | Out-Null
Copy-Item "$rel\handy.exe" $out; Copy-Item "$rel\*.dll" $out; Copy-Item "$rel\resources" "$out\resources" -Recurse
foreach ($d in 'msvcp140*.dll','vcruntime140*.dll','vcomp140.dll','onnxruntime.dll') { Copy-Item "$env:LOCALAPPDATA\Handy\$d" $out -ErrorAction SilentlyContinue }
Set-Content -Path "$out\portable" -Value 'Handy Portable Mode' -NoNewline
$zip = "C:\dev\Handy-0.9.7-hotkeys.1-$sha-portable.zip"
Compress-Archive -Path "$out\*" -DestinationPath $zip -Force
Get-Item $zip | Select Name,Length,LastWriteTime | Format-List
(Get-FileHash $zip -Algorithm SHA256).Hash
"PORTABLE DONE"
