[![build](https://github.com/antjowie/project-odyssey/actions/workflows/rust.yml/badge.svg)](https://github.com/antjowie/project-odyssey/actions/workflows/rust.yml)
# Project Odyssey

A train simulator "game"?

### Todo

Rail editor
- [x] Rail planner placement iteration
- [x] Setup rail planner validations
- [x] Reintroduce snapping to end_intersection if one is hovered
- [x] Generate procedural mesh from spline
- [x] Gen collider for curve
- [x] Add segment deconstruction
- [ ] Add intersection deconstruction
  - I don't think this is possible with current systems, as too many edge cases
  - are introduced. Current system relies directly on splines, but it might be
  - better to think of a "preset matching" system next time, so we can easily
  - move things around. 
  - Issue is that a rail is assumed to be 1 curve, so removing an intersection
  - means we need to merge 2 curves, we could then split this joined curve 75%
  - through, which brings us in all kinds of weird situations.
- [x] Improve joint expansion to intersection instead of seperate joints
- [x] Insert rail into arbitrary area of rail
- [x] Expand rail from arbitrary area of rail
- [ ] Add vertical rail building
  - [x] Add proper raycasting
- [ ] Add copy pasta? 

Train
- [x] Support placing and creating different things on rails
- [x] Add train and have them drive
- [ ] Add stations
- [ ] Support specifying stations for trains
- [ ] Support train moving along a planned route
- [ ] Respect traffic groups
- [ ] Collision response

Input
- [x] Use context based input components (this can control state)
- [x] Write context based input components to screen

Pathfinding
- [ ] Generate nav graph for rail
- [ ] Support setting destinations
  - Need to somehow know which node depending on which segment we hovered
- [ ] Add trafic control via signals
  - I'm thinking of each link having a traffic id which maps to a map
  - Then in the map we track which trains are on which tracks, so we can use this to calc weights and such

General gameplay
- [ ] Add save and load

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

### 1.15 bevy migration
Migration
1. VBAO https://bevyengine.org/news/bevy-0-15/#visibility-bitmask-ambient-occlusion-vbao
2. Entity picking https://bevyengine.org/news/bevy-0-15/#entity-picking-selection
3. Bubbling https://bevyengine.org/news/bevy-0-15/#bubbling-observers
4. Curves https://bevyengine.org/news/bevy-0-15/#curves
5. Function reflection https://bevyengine.org/news/bevy-0-15/#function-reflection
6. Custom cursors https://bevyengine.org/news/bevy-0-15/#custom-cursors

### Some nice resources
* [Rust book](https://doc.rust-lang.org/book/)
* [Bevy book](https://bevy-cheatbook.github.io/)
* [Tainted Coders (bevy guide/ref)](https://taintedcoders.com/)
  * [Awesome Bevy (repo of info)](https://github.com/nolantait/awesome-bevy)
* [ECS Guide](https://github.com/bevyengine/bevy/blob/v0.14.0/examples/ecs/ecs_guide.rs)
* [Bevy Examples](https://bevyengine.org/examples/)

### Some rust/bevy pain points
* Debugger experience is subpar. A vec of dyn objects gives pretty much no info (pointer to pointer to pointer, nothning concrete) As does a Res type. It might be due to opt-levels but I can't put it lower cuz I run into linker limitations, why is the limit a 16bit integer anyway?
  * For example, our input vec of type Buttonlike gives us `vec->buf->inner->ptr->pointer->pointer->*pointer = 0`... I'd expect some more concrete data but maybe the external lib just does some crazy stuff that I have to dive a bit deeper into
* Unable to easily browse symbols of dependencies, I gotta write the type and jump to it, I'd like to just ctrl+t and search for anything
  * Upon further investigation this is a setting that can be configured `"rust-analyzer.workspace.symbol.search.scope": "workspace_and_dependencies"`. Unfortunately it is very slow and even worse, I can no longer find my own symbols