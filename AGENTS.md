# terra-cube

An experiment: a voxel sandbox game with heavy simulation, better looking than Minecraft (Eco is the reference), built in Rust by one developer and coding agents. Every model in it is generated. This file is the map; the docs it points to are the record. Keep it under 120 lines.

## Where things stand

- **Works:** the creature pipeline (`cargo forge`): one line of text to an animated model the game loads, checked by numbers and by one picture. The world (`make play`): an island a kilometre across made from a seed and held in cells only near the player, with a calendar (day, seasons, a moon), an ecology that places some two thousand creatures and the small life round the camera, sound, and a player with health, a pack, a sword and a torch. `cargo world` judges both land and life with no window. The sandbox (`make run`) poses any creature.
- **Baseline only:** the world is 0.25 m cells (each able to break into 5 cm bits), drawn in blocks sized to the slope; trees, rocks, grass and flowers are small cubes of many sizes on it, under a scattering sky. Destruction is by tool: a shovel digs earth, an axe fells a tree and works it down to hunks of wood, a pick breaks a boulder into hunks of stone; a hunk is carried in the hands, one at a time, and put down as cells. Building and crafting are not begun. Nothing is saved, no networking.
- **Next:** [docs/engine.md](docs/engine.md#order-of-work).

## Layout

| Path | What it is |
|---|---|
| `crates/cube_voxel` | Voxel data (cells, and the bits a cell breaks into) and chunk meshing. No engine types, so the simulation can use it headless. |
| `crates/cube_sim` | The simulation, at a fixed 60 ticks a second, no engine types: how big everything is (`scale`: the one place sizes live), the world from a seed (`terrain`), where and when each kind of life lives (`ecology`), bodies moving through voxels (`body`), what grows on it (`plants`), every creature and its mind (`life`, `mind`). |
| `crates/cube_anim` | A body moved by its own skeleton and feet, no clips: two-bone inverse kinematics, feet that plant and step, a trunk carried by them; on four legs (`Walker`) and on two (`biped`: `cube_game::stride` moves a person marked `walked = [..]` by it; the player is not, since 2026-11-14, and moves by recorded clips). Plain arithmetic with tests; a trial on boxes (`make limbs`, `make person`) before any real model. |
| `crates/cube_game` | The Bevy app: draws what the simulation holds, takes the player's keys. Sky, light, wind and what drifts in the air (`world`, `air`). Also the sandbox panel, contact sheets, scene pictures. |
| `crates/world` | `cargo world`: the land and its creatures judged headless (`survey`, `run`, `life`, `map`, `grade`, `who`, `dig`). |
| `crates/forge` | The creature pipeline CLI. Each file's header says what it holds; `main.rs` has the map. |
| `tools/audio` | Sound judged by measurement (ffmpeg): `level.py` brings every file to one loudness, `sources.py` checks each has its source and licence in the README, `loops.py` joins loops, `listen.sh` and `mix.py` judge five minutes of what the game played, `cut.py` and `cut.sh` cut calls from recordings. |
| `tools/xkeys` | Real key presses and clicks on an X display, and `make window`'s test of the game's window with them. Python and libXtst, nothing to install. |
| `tools/library.sh` | Fetches the hand-animated human motion (CC0) forge carries onto Tripo's people, to `.work/library/`. Needed before `cargo forge finish` of a biped. |
| `tools/tripo` | Tripo Studio CLI (TypeScript, Node ≥ 22.18, no dependencies). Forge drives it; see its README before calling it by hand. |
| `assets/textures/` | Photographed surface materials (CC0), one per grain; its README says where each came from. |
| `assets/shaders/` | Wind in what grows: two vertex shaders that must be changed together ([engine.md](docs/engine.md#wind-and-the-air)). `cloud.wesl`: the light inside a cloud ([engine.md](docs/engine.md#light)). |
| `assets/creatures/style.toml` | The art style and body plans, as forge sends them. Change the look here, not per creature. |
| `assets/creatures/<id>/` | `creature.toml` (written by hand; `[life]` is how it behaves and where it lives), `forge.lock` (what Tripo made), `meta.toml` and `model.glb` (made by forge; never edit). |
| `docs/` | [engine.md](docs/engine.md) design and order of work · [pipeline.md](docs/pipeline.md) making creatures · [art-guide.md](docs/art-guide.md) how creatures and the land should look, and what was tried and dropped · [development.md](docs/development.md) setup and commands · `research/` dated notes with sources. |
| `.work/`, `shots/` | Scratch: Tripo downloads and rendered sheets. Ignored by git; safe to delete. |

## Commands

`make` alone checks the machine is set up; `make help` lists the everyday ones (`make setup`, `make run`, `make play` (both open a Windows build when run in WSL), `make world`, `make map`, `make sheet ID=wolf`, `make check`). Each is one line over these; add a target when a new command becomes everyday.

```bash
cargo forge doctor                 # is this machine set up: node, Tripo login, display, models
cargo sandbox --help               # every way to start the game (--list names the creatures)
cargo forge status                 # every creature: Tripo steps done, credits, model
cargo forge build wolf             # make what is missing, then finish and check (spends credits once)
cargo forge finish --all           # the free local half again, after changing clips.rs or style.toml
cargo forge sheet wolf             # shots/wolf.png: every clip at six moments, one image
make legs                          # the player up and down a hillside and on a ledge: how far under the drawn ground a knee or ankle goes (`cargo world drawn`: how far the drawn ground is from the cells a body stands on)
make feet                          # feet put on the ground that is there (`cube_game::feet`): how far the standing foot strays on a hillside, as the clip has it and planted; `--loose-feet` on any `cargo sandbox` turns it off
make held                          # shots/held.png: the sword and torch in the player's hand through standing, walking and a blow; after any change to `assets/world/held.toml` or a biped's clips
make moves ID=wolf                 # the routine for a four-legged creature's movement: forge walks the model and bakes its gaits, judges them by number, draws one side sheet
make limbs                         # shots/limbs.gif: a four-legged body of boxes moved by its skeleton and feet, no clips; `make person`: a person the same way. Fix motion on the boxes before any model
make beside                        # shots/beside.gif: the hand-animated library's own figure and ours through the same keys, side by side; look before blaming the clips or the game
make gaits ID="wolf deer"           # shots/gaits.png: walk, run and sprint from the side, large; look after any change to motion
cargo world survey                 # the land in numbers; `cargo world run`: its creatures living, in numbers
cargo world life                   # the ecology: how much of the island suits each kind of life, when it is about, where each animal settled
cargo world years 24               # the island by the day for 24 years, nobody watching: each kind's numbers, whether it lasts and settles, who hunted what fell; change `[life]` or `Sim::day` against this, on more than one `--seed`
cargo world fell                   # a tree near the start felled with an axe, no window: it stands to its last blow, comes loose, goes over, lies with its trunk on the ground; `fell X,Z` the tree nearest a place, `make fells` 25 of them (one tree says little: big ones did not fall); `cargo world hunks`: a tree and a boulder worked to the pieces that are carried (felled, limbed, broken, split; chipped, broken), on any `--seed`; `cargo world joined`, `cargo world quarry`, `cargo world lying` (how far what a body stands on is from a log drawn at its angle)
make forget                        # the same in the real game: trees felled and cut up, every changed region dropped and made again from seed and journal (a script's `forget`), 0 cells may differ; `LOOSE_TRACE=1` prints each cut and what kinds of cell differ; `make dig`: the ground dug with the shovel and the earth put back, the same way; `make shatter`: a stone let go five metres up breaks and lies as rubble; `make build`: a tree worked to hunks, four carried and stacked; `make stone`: a rock picked to hunks, one carried; `make trees`: four trees, two of them great, worked and timed
cargo world replay                 # edits played on real land, the land made again from its seed, the edits done again: 0 cells may differ (`loose::replay`: a world is its seed and its journal)
cargo world trunks                 # how far what stops a body at a tree is from the wood that is drawn (0 cm at the middle; it was 15); `cargo world roots`: how far the wood at a tree's foot hangs over the ground under it
cargo world map shots/map.png      # one picture from above with where every creature went
make window                        # the real window on an Xvfb, driven by real key presses (tools/xkeys), judged from the trace
make prove                         # every judgment in one go, a line each: check, survey, run, life, sound levels, what is heard, fight, swim, window, seasons
make sound                         # five minutes heard in a meadow by day and a shore from dusk, judged from the trace: files, repeats, level against a footfall (`tools/audio/listen.sh forest 21` for any place and hour)
make pace                          # the player run across the world on the graphics card: load time and every frame's time while he ran, against bars (`make digpace`: the same while he digs sixty holes; `cargo world cost`: what a region and a mesh cost, by part)
make swim                          # the player put in deep water in the real game: floats, swims, is heard (`cargo world find deep` finds it, `find boulder` a big stone)
make fight                         # the player fights a goblin in the real game, judged from the trace: hit, kill, sounds
make grown                         # shots/grown/*.png: every kind of tree and rock alone, whole and close; `make floors`: the ground of a trail, forest, meadow and shore in one sheet
make wind                          # a wood half a second apart in still air and a hard wind: leaves must move in one, not the other (`--wind 0.8` on any `cargo sandbox` sets it)
make sky                           # shots/sky.png: the sky and the light at twenty fixed hours and places (dawn, noon, sunset, dusk, moonrise, night, the clouds, the torch, a wood, a lake), one sheet; look at it after any change to sky, sun, moon, cloud, exposure or a light source (`--view sky` looks out over the trees)
make cave                          # shots/cave.png: light under the ground, ten views of a cave cut into a hillside for the purpose (`--cave`, `--fire`; the land has none of its own): by day, the sun down the hole in its roof, by torch, and at night by a torch and by a fire; after any change to exposure, the sky's fill or the torch (`--eye X,Y,Z --look X,Y,Z` puts a scene's camera anywhere)
make slice                         # shots/slice/*.png: six fixed views (meadow at noon, dawn and night, forest, shore, from the air); look at all six after any change to the world's look
cargo world grade shots/grade.png  # where the ground is easy, steep and cliff, by height and in a picture
cargo sandbox --spawn all          # the interactive sandbox (needs a real screen)
cargo sandbox --play --wild        # the living world, as the player (make play)
cargo sandbox --scene shots/herd.png --wild --at 47,28   # no window: a picture of any place (`cargo world who` says where things live); `--hour 6.2` for any time of day
cargo sandbox --scene shots/deer.gif --watch deer --at -32,-40 --gif 8   # no window: the camera kept on the nearest deer living by its own mind (a place alone found empty ground three times in four)
cargo sandbox --scene shots/wolf.gif --play --as wolf --gif 12 --view three   # no window: a moving picture of every gait and change of clip, for a person to watch (make gif ID=wolf)
cargo sandbox --scene shots/run.png --play --script '0.5 fwd run; 2 jump' --strip 10 --after 1.5 --view side   # no window: a scripted run as a strip of pictures
cargo test --workspace             # must pass before a commit
cargo clippy --workspace -- -D warnings
```

`ANIM_TRACE=player` with any `--scene --play` prints what that creature's clip is doing (which, where in it, how fast); `FORGE_TRACE=1` with `cargo forge finish ID` says of each clip where its skin stretches most (`stretch NAME where`); `FLORA_TRACE=1` with any `--scene` prints what each region's trees and rocks cost in faces, by level and part: add it up before guessing where a slow frame comes from. `SLOW_TRACE=1` with any `--scene --play` prints each frame over 20 ms with what was made in it, and each of our systems that took over 8 ms (`crate::Slow`); the pace line says when the slowest frame was.

A machine with no screen needs `DISPLAY=:1` (an Xvfb) for `sheet` and `sandbox`; see [docs/development.md](docs/development.md).

## Rules

- **Bevy 0.20, pinned.** Its API changes every release and your memory of it is probably older. Before writing engine code, read the matching example in `~/.cargo/registry/src/*/bevy-0.20.0/examples/` rather than guessing; grep the source for a signature. Same for `bevy_egui` 0.43.
- **Repeatable work is code.** If you did something by hand twice (a Tripo call, a GLB fix, a render), it becomes a forge command or a step of one. Never hand-edit a generated file (`model.glb`, `meta.toml`); change the code or `style.toml` and run `cargo forge finish`.
- **What you measure with becomes a tool.** A probe written to find out why something is wrong (a histogram, a map, a trace) will be wanted again: make it a `cargo world` or `cargo forge` command with a line of help, not a script thrown away.
- **Tools answer in a few lines.** A command prints a verdict and numbers, `ok` or `FAIL name value (bar)`, and names a file for details. Passing checks stay quiet. Keep new tools that way: output is read by an agent on every later turn.
- **Numbers first, then one picture.** `cargo forge check` judges a model before anyone looks. Then look at one contact sheet (`cargo forge sheet`), not a run of screenshots. If you catch a defect by eye that the checks missed, add a check for it. Movement is judged the same way: one `make gaits` sheet beside what the clip came from, then one `--strip`; a passing number has twice hidden bad motion ([pipeline.md](docs/pipeline.md#judging-motion)). Every `--scene` shot ends with its `pace` (frame time, faces in view): look at it after any change to what is drawn. So are the world and behaviour: `cargo world survey` and `run`, then one `map`. Change terrain or minds against those, not by playing.
- **Credits are real money.** `forge.lock` makes a build spend once. Look at the concept (`cargo forge build ID --to concept`, then `cargo forge concepts`) before paying for a model. Never delete a lock step to retry without a reason you can state.
- **A person's motion is a person's, untouched.** Tripo's presets and the hand-animated library (`cargo forge finish` carries its clips onto a Tripo humanoid: [pipeline.md](docs/pipeline.md#where-motion-comes-from)) are used as recorded; animals take a hand-animated wolf's by their feet; forge makes a clip only where none of these has one (a trot, a hop, grazing). Do not rewrite a recorded clip, and do not pose over one.
- **Bones have roles, not names.** Tripo's bone names are unreliable; clips are written against roles read from the skeleton's shape (`crates/forge/src/rig.rs`). Do not match on bone names for animals. The two exceptions: Tripo's `Head_n` and `Tail_n` say which chain is the head and which the tail (shape had a squirrel's brush for its head, and it ran tail first), and a humanoid's bones are Mixamo's.
- **Runs on Linux, Windows and macOS.** Build paths with `Path::join`, never a typed `/` or a home folder; no shell-outs beyond `node` and `cargo`; anything one system needs sits behind `cfg!` with the reason. `make check` type-checks for Windows. In WSL the game is built as a Windows program and opened there (`make play` does it), so it must start from any folder: it finds `assets/` itself.
- **Nothing long on the frame's thread.** Making cells, meshes and tree shapes goes to the task threads and the frame takes in what is finished ([engine.md](docs/engine.md#a-big-world)); measure with `make pace` before and after anything that touches what is made or drawn, and profile before guessing (three guesses in a row were wrong the day this was written).
- **The simulation must not need a window.** Game rules go in crates with no rendering (`cube_voxel`, `cube_sim`), tested headless. `cube_game` only draws and takes input: it never moves a creature itself. Anything random comes from the `Sim`'s seeded generator, so a seed replays.
- **Secrets:** `.env` (the Tripo login) lives in the main checkout, ignored by git. Never print it or commit it.
- **Shaders** are for the environment only (terrain, water, sky, weather). Creatures use stock materials.
- **Small commits, often** (the developers): land each step that works and is checked on its own (a test, a fix, a measured change, a tool) as soon as it is done, one change a commit, tests and clippy passing; never squash a job's work into one commit or hold it to the end. Unchecked work stays in the worktree. Models and textures are committed for now (a few hundred KB each); see the open question in engine.md before adding anything over 5 MB.
- **Push every commit to main, without asking; pull just as often.** Several agents and people work on main at once, so the less time between pulls, the less anyone diverges. After a commit: `git pull --rebase origin main`, `git push origin HEAD:main`, again if main moved; then fast-forward the main checkout (`git pull --ff-only` there) so worktrees cut from it are not stale. Pull before starting work and before each new step too. No pull requests, never force-push main, never `--no-verify`.
- **Docs:** when a design choice is made, write it in `docs/engine.md` or `docs/pipeline.md` under the heading it belongs to, with the reason. Research goes in `docs/research/YYYY-MM-DD-topic.md` with sources. Do not write plans or status files.

## Adding things

- **A kind of tree, or what the ground is and what grows on it:** data, not code. A tree is a table in `assets/world/flora.toml`; ground and cover are rules in `assets/world/land.toml`, densities and undergrowth too. Then `cargo world survey`, `make grown`, `make floors`.
- **Where and when anything lives:** its niche. For an animal, `habitat`, `water`, `cover` and `hours` in `[life]`; for small life the game draws, a `Niche` in `crates/cube_sim/src/ecology.rs`. Never a placement rule in the code that draws it. Then `cargo world life`.
- **A creature that lies down to rest:** look at `cargo sandbox --shot ID --clips lie --view side` first; `lies = true` in its `creature.toml` only if that is an animal lying down.
- **A pose or a motion is not invented.** Look at photographs of the real animal first, write what makes it in `assets/creatures/poses.toml` as measures, build to them, let forge measure it, look once, and add a measure when the eye catches what the numbers passed (the process is at the head of that file).
- **What every creature must be able to do is written down** (`assets/creatures/requirements.toml`), and `make features` holds each to it; `seen.toml` beside it lists the clips somebody has looked at and found right. A new ability goes in the first; a clip goes in the second only after looking.
- **How a creature behaves:** `[life]` in its `creature.toml`; a new temper or state in `crates/cube_sim/src/mind.rs` with a test beside the others in `life.rs`, then `cargo world run`.
- **A creature:** [docs/pipeline.md](docs/pipeline.md). Four commands, about 67 credits. A four-legged animal: `plan = "walked"` (our skeleton in Tripo's mesh). A person: the library's recorded clips, mixed by speed, feet put on the ground (`walked = [..]` gives it to the two-legged walker instead: tried on the player and dropped, [pipeline.md](docs/pipeline.md#what-motion-has-to-be-before-any-of-it-is-tuned)). Both sections say what to check.
- **The world is cuboids, all of it** ([art-guide.md](docs/art-guide.md)): detail comes from small cubes of many sizes and shades, never from smooth shapes or flat planes. `--shapes` and `--blades` show what was set aside.
- **Creatures are not made of cubes.** That was tried and parked (`cargo forge cubes`, [docs/pipeline.md](docs/pipeline.md#cubes)); They fit the world by style, and the style is the house style in `style.toml`: other looks were tried and it was kept.
- **A bird, an insect or a fish:** `[built]` in its `creature.toml` and `cargo forge finish ID`: forge makes it of boxes and rigs it itself, free ([docs/pipeline.md](docs/pipeline.md)); `cube_game::air` flies it. **What crawls** (six or eight legs) is `form = "crawler"`, **a snake** `form = "serpent"` (look at it with `--view top`) and **a bird on the ground** `form = "fowl"`, the same way; each walks the ground as any creature.
- **A clip for a body plan:** a recipe in `crates/forge/src/clips.rs`, its name in the plan's `clips` in `style.toml`, then `cargo forge finish --all`.
- **A body plan (bird, spider, snake):** a `[plans.x]` in `style.toml`; the role reader and recipes are plan-agnostic, so check `cargo forge inspect ID` reads the new shape sensibly first.
