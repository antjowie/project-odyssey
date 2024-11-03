set "param1=%1"
if not defined param1 cargo run --profile dev-nodebug --features bevy/dynamic_linking
if %1 == "debug" cargo run --features bevy/dynamic_linking
if %1 == "web" cargo run --target wasm32-unknown-unknown
