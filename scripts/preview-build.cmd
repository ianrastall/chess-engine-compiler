@echo off
"%~dp0chess-engine-compiler.exe" plan "%~dp0engine.json"
exit /b %errorlevel%
