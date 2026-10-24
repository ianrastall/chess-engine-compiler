@echo off
"%~dp0chess-engine-compiler.exe" build "%~dp0engine.json"
exit /b %errorlevel%
