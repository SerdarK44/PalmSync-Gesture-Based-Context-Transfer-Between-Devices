@echo off
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo This script must be run as Administrator.
    pause
    exit /b 1
)

echo Installing PalmSync service...
"%~dp0palmsync-daemon.exe" install
if %errorlevel% neq 0 (
    echo Service installation failed.
    pause
    exit /b 1
)

echo Starting PalmSync service...
net start PalmSync
if %errorlevel% neq 0 (
    echo Service start failed.
    pause
    exit /b 1
)

echo PalmSync service installed and started successfully.
pause
