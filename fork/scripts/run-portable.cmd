@echo off
call "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"
powershell -NoProfile -ExecutionPolicy Bypass -File C:\Users\frank\build-portable.ps1
