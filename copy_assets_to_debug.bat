@REM RustRover working dir is where the build files are (target\debug)

xcopy "%~dp0assets\*" %~dp0target\debug\assets\ /s /y