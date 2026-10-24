@echo off
"%~dp0chess-engine-compiler.exe" build "%~dp0stockfish.json"
exit /b %errorlevel%
