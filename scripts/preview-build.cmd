@echo off
"%~dp0chess-engine-compiler.exe" plan "%~dp0stockfish.json"
exit /b %errorlevel%
