[![build](https://github.com/antjowie/project-odyssey/actions/workflows/rust.yml/badge.svg)](https://github.com/antjowie/project-odyssey/actions/workflows/rust.yml)
# Project Odyssey

A train simulator "game"?

### Todo

Rail editor
- [] Setup rail planner validations
- [] Setup basic UI, mostly for feedback
- [] Add vertical rail building
- [] Extend rail arbitrary from segment
- [] Add segment deconstruction
- [] Add copy pasta? 

Pathfinding
- [] Generate nav graph for rail
- [] Support setting destinations
  - Need to somehow know which node depending on which segment we hovered
- [] Add trafic control via signals
  - I'm thinking of each link having a traffic id which maps to a map
  - Then in the map we track which trains are on which tracks, so we can use this to calc weights and such

General gameplay
- [] Add save and load

Visuals
- [] Generate procedural mesh from spline

### Build
You can use cargo as you always would, simply `cargo run` would suffice.

For some different options you can check [run.bat](run.bat) which I use when developing:
* Run `run.bat` for fastest iteration times
* For testing web builds 
  * Prereqs
    * Run `cargo install wasm-server-runner` 
    * Run `rustup target add wasm32-unknown-unknown`
  * Now anytime you want you can run `run.bat web`

### Attaching debugger
I use VSCode for development. If you want to attach a debugger you can F5. Make sure `stable-x86_64-pc-windows-msvc` is installed (run `rustup toolchain list`) or check [launch.json](.vscode/launch.json) to update according to your needs

### Multiplayer
> I'll drop multiplayer for now, while I still keep it in mind let's not try and learn about bevy by making a fully deterministic simulation :#
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