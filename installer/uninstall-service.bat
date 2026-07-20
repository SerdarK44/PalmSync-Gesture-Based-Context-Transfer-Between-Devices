@echo off
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo This script must be run as Administrator.
    pause
    exit /b 1
)

echo Stopping PalmSync service...
net stop PalmSync >nul 2>&1

echo Removing PalmSync service...
"%~dp0palmsync-daemon.exe" uninstall
if %errorlevel% neq 0 (
    echo Service removal failed.
    pause
    exit /b 1
)

echo PalmSync service removed successfully.
pause
