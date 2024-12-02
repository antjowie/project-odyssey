[![build](https://github.com/antjowie/project-odyssey/actions/workflows/rust.yml/badge.svg)](https://github.com/antjowie/project-odyssey/actions/workflows/rust.yml)
# Project Odyssey

A train simulator game

### Milestones

MS1 - a train simulation

- [] Setup camera and world
- [] Add build system with grid and items
- [] Setup workflow for multiplayer development
- [] Add save and load
- [] Add rails and trains
- [] Add signals to rails

MS2 - gameplay progression

- [] Add resources
- [] Add logistics to move resources around
- [] Come up with a reason to drive progression
  - I'm thinking something like workers or drones being moved to stations, and them collecting resources


### Build
You can use cargo as you always would, simply `cargo run` would suffice.

For some different options you can check [run.bat](run.bat) which I use when developing:
* Run `run.bat` for fastest iteration times
* For testing web builds first install `cargo install wasm-server-runner` then `run.bat web`

### Attaching debugger
I use VSCode for development. If you want to attach a debugger you can F5. Make sure `stable-x86_64-pc-windows-msvc` is installed (run `rustup toolchain list`) or check [launch.json](.vscode/launch.json) to update according to your needs

### Multiplayer
Client-server, but considering if we want:
* Clients to locally simulate and use checksum to validate (Factorio does CRC).
  * Fully simulating means we need to rely on deterministic behavior. So think about float precision issues.
    * Best to rely on integers for game state, or round them, or fixed point floats.
    * If we have items on the belt, how would we verify their pos? Maybe round it?
  * Cross platform concerns.
* Clients to sync up via replicating changes changes.
  * Scales worse.
  * Easier to get moving.

Maybe a mix between the 2? I feel fully deterministic could be more trouble then worth it...
If network really ends up being a blocker, I could always do local simulation and resolve desyncs by reloading the game in clients :P
https://www.youtube.com/watch?v=ueEmiDM94IE&t=2235s

### Some nice resources
* [Rust book](https://doc.rust-lang.org/book/)
* [Bevy book](https://bevy-cheatbook.github.io/)
* [Tainted Coders (bevy guide/ref)](https://taintedcoders.com/)
  * [Awesome Bevy (repo of info)](https://github.com/nolantait/awesome-bevy)
* [ECS Guide](https://github.com/bevyengine/bevy/blob/v0.14.0/examples/ecs/ecs_guide.rs)
* [Bevy Examples](https://bevyengine.org/examples/)