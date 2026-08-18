@echo off
REM SPDX-License-Identifier: CC-BY-NC-SA-4.0
REM
REM VeilVoice installer for Windows -- a wrapper, so that double-clicking works.
REM
REM All of the work, and every one of the checks, is in install.ps1 next to this
REM file. This exists only because .bat is what Windows runs on a double-click
REM and because `powershell -ExecutionPolicy Bypass -File ...` is a mouthful to
REM type. Keeping the logic in one place matters here more than usual: two
REM implementations of a verification routine means one of them is the stale one,
REM and the stale one is the one that will be running when it matters.
REM
REM   install.bat                 interactive
REM   install.bat -Yes            no prompts, no optional components
REM   install.bat -Version v0.1.9
REM
REM Arguments are passed through to install.ps1 unchanged.

setlocal

set "SCRIPT=%~dp0install.ps1"

if not exist "%SCRIPT%" (
    echo.
    echo REFUSED: install.ps1 was not found next to this file.
    echo.
    echo   Expected: %SCRIPT%
    echo.
    echo This wrapper does nothing on its own -- every check lives in that
    echo script. Download the whole install directory, not just this file.
    echo.
    exit /b 1
)

REM -NoProfile: a profile script can redefine anything, including the cmdlets
REM used to verify the download. -ExecutionPolicy Bypass applies to this process
REM only and changes no machine setting.
powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%" %*
set "RC=%ERRORLEVEL%"

REM A double-clicked window closes the moment the script ends, taking any
REM refusal message with it. Pause only when there is nobody to read the exit
REM code -- i.e. when this was not run from an existing console.
echo.
if not "%RC%"=="0" (
    echo Installation did not complete. The reason is above.
)
if "%CMDCMDLINE:~0,7%"=="cmd /c " pause

exit /b %RC%
