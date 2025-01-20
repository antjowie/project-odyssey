@REM Sometimes the run fails due to bevy_dylib dependency. 
@REM Clearing the cache resolves this.
@REM https://github.com/bevyengine/bevy/issues/17361

del /s /q "%UserProfile%\.cargo\registry\*"