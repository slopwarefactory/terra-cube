# The everyday commands, so nobody has to remember the cargo ones. `make` alone checks the
# machine is set up; `make help` lists them.
# Each is one line over `cargo forge`, `cargo sandbox` or `cargo world`; those take more options (--help).

SPAWN ?= all
ARGS ?= --play --wild
WINDOWS := x86_64-pc-windows-gnu
# In WSL the game is built for Windows and opened there: the real GPU, a native window.
WSL := $(shell grep -qi microsoft /proc/version 2>/dev/null && echo 1)

.DEFAULT_GOAL := setup
.PHONY: cave digpace forget dig shatter build stone trees fells sky wind blow moves limbs person beside features legs feet held pace gaits sound years seasons grown floors prove swim window fight slice help setup run play windows list scene sheet gif status creature finish world map test lint cross check

help: ## this list
	@awk -F ':.*## ' '/^[a-z]+:.*## / { printf "make %-22s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

setup: ## (plain `make`) check this machine: node, Tripo login, display, models
	cargo forge doctor

run: ## the sandbox window, one of each creature to pose and inspect (SPAWN=wolf,player for some)
ifeq ($(WSL),1)
	@$(MAKE) --no-print-directory windows ARGS="--spawn $(SPAWN)"
else
	cargo sandbox --spawn $(SPAWN)
endif

play: ## the living world, as the player: WASD, Shift sprints, Ctrl walks, Space jumps, V first person, Tab frees the mouse
ifeq ($(WSL),1)
	@$(MAKE) --no-print-directory windows ARGS="--play --wild"
else
	cargo sandbox --play --wild
endif

windows: ## build for Windows and, in WSL, open it there; `make play` and `make run` do this themselves in WSL
	@command -v x86_64-w64-mingw32-gcc >/dev/null || { echo "needs the Windows linker: sudo apt install mingw-w64"; exit 2; }
	CARGO_PROFILE_RELEASE_DEBUG=false CARGO_PROFILE_RELEASE_STRIP=true cargo build --release --package cube_game --target $(WINDOWS)
	@if grep -qi microsoft /proc/version 2>/dev/null; then target/$(WINDOWS)/release/cube_game.exe $(ARGS); \
	else echo "built target/$(WINDOWS)/release/cube_game.exe; this is not WSL, so it was not started"; fi

list: ## the creatures that can be spawned
	@cargo sandbox --list 2>/dev/null

scene: ## no window: shots/scene.png, the sandbox with creatures in it
	cargo sandbox --scene shots/scene.png --spawn $(SPAWN)

world: ## no window: is the world sound? the land, then five minutes of its creatures, in numbers
	cargo world survey
	cargo world run --seconds 300

map: ## no window: shots/map.png, the island from above with where each creature went
	cargo world map shots/map.png

# The vertical slice: the same six places at the same hours, every time. Look at all of them
# after any change to how the world is drawn; a view that got worse is a regression.
SLICE = cargo sandbox --wild --scene shots/slice
slice: ## no window: shots/slice/*.png, seven fixed views and two moving ones, to judge any change to its look against
	$(SLICE)/meadow-noon.png --play --script '2 fwd walk' --after 3 --view three --hour 13
	$(SLICE)/meadow-dawn.png --play --script '2 fwd walk' --after 3 --view three --hour 6.4
	$(SLICE)/meadow-night.png --play --script '2 fwd walk' --after 6 --view three --hour 21
	$(SLICE)/meadow-torch.png --play --script '2 fwd walk' --after 6 --view three --hour 21 --torch
	$(SLICE)/forest-day.png --at 19,34 --dist 10 --pitch 0.1 --hour 15 --after 8
	$(SLICE)/shore-evening.png --at 48,19 --dist 14 --pitch 0.3 --hour 17
	$(SLICE)/island-high.png --view high --hour 15
	$(SLICE)/meadow-autumn.png --play --script '2 fwd walk' --after 3 --view three --day 17 --hour 13
	$(SLICE)/meadow-winter.png --play --script '2 fwd walk' --after 3 --view three --day 24 --hour 13
	$(SLICE)/hud-pack.png --play --script '2 fwd walk' --after 3 --view three --hour 13 --hud pack
	$(SLICE)/hud-admin.png --play --script '2 fwd walk' --after 3 --view three --hour 13 --hud admin
	$(SLICE)/far-high.png --at 0,0 --view high --dist 420 --pitch 0.4 --hour 13
	$(SLICE)/forest-moving.gif --at 19,34 --dist 10 --pitch 0.1 --hour 15 --gif 6
	$(SLICE)/meadow-walk.gif --play --script '1 fwd walk; 3 fwd run; 5 stop' --gif 7 --view three --hour 13

prove: ## every judgment there is, a line each: code, land, creatures, ecology, sound levels, what is heard, a fight, a swim, the real window
	@$(MAKE) --no-print-directory check > /dev/null 2>&1 && echo "ok check: tests, clippy, the Windows build" || echo "FAIL check (make check)"
	@cargo world survey | tail -1 | sed 's/^ok$$/ok survey: the land/'
	@cargo world run --seconds 300 | tail -1 | sed 's/^ok$$/ok run: five minutes of creatures/'
	@cargo world life | tail -1 | sed 's/^ok$$/ok life: every kind has somewhere to live/'
	@cargo world trunks | tail -1 | sed 's/^ok$$/ok trunks: what stops a body at a tree is the wood that is drawn/'
	@for seed in 6 1 11; do cargo world fell --seed $$seed | grep -E '^FAIL' | sed "s/^/seed $$seed: /"; done; cargo world fell | tail -1 | sed 's/^ok$$/ok fell: a tree cut through comes down whole and lies where it fell, as cells/'
	@tools/fells.sh 30
	@for seed in 1 7; do cargo world quarry --seed $$seed | grep -E '^FAIL' | sed "s/^/seed $$seed: /"; done; cargo world quarry | tail -1 | sed 's/^ok$$/ok quarry: a boulder dug out from under comes down, breaks if it comes down hard, and its stone lies where it fell/'
	@for seed in 5 7; do cargo world hunks --seed $$seed | grep -E '^FAIL' | sed "s/^/seed $$seed: /"; done; cargo world hunks | tail -1 | sed 's/^ok$$/ok hunks: a tree and a boulder worked to the pieces that are carried, each whole and lying on something, one taken up and put down/'
	@for seed in 5 7; do cargo world lying --seed $$seed | grep -E '^FAIL' | sed "s/^/seed $$seed: /"; done; cargo world lying | tail -1 | sed 's/^ok$$/ok lying: what a body stands on is the log that is drawn, lying at its angle, to 8 cm, and a person walks and jumps over each/'
	@for seed in 1 7; do cargo world roots --seed $$seed | grep -E '^FAIL' | sed "s/^/seed $$seed: /"; done; cargo world roots | tail -1 | sed 's/^ok$$/ok roots: the wood at the foot of every tree near the start comes down to the ground under it/'
	@for seed in 1 3; do cargo world joined --seed $$seed | grep -E '^FAIL' | sed "s/^/seed $$seed: /"; done; cargo world joined | tail -1 | sed 's/^ok$$/ok joined: every tree near the start, as the cells it is once cut into, is one body with its foot/'
	@cargo world replay | tail -1 | sed 's/^ok$$/ok replay: the land made again from its seed with the same edits done to it is the same world/'
	@tools/forget.sh
	@tools/dig.sh
	@tools/shatter.sh
	@tools/build.sh
	@tools/stone.sh
	@tools/trees.sh
	@tools/digpace.sh
	@cargo world drawn | tail -1 | sed 's/^ok$$/ok drawn: a body rests on the ground as it is drawn/'
	@python3 tools/audio/level.py | tail -1
	@python3 tools/audio/loops.py | tail -1
	@python3 tools/audio/sources.py
	@python3 tools/features.py
	@$(MAKE) --no-print-directory sound
	@$(MAKE) --no-print-directory fight
	@$(MAKE) --no-print-directory swim
	@$(MAKE) --no-print-directory window
	@tools/pace.sh --yaw 0.7
	@tools/feet.sh player -60,20 2.0
	@tools/legs.sh
	@tools/seasons.sh
	@tools/wind.sh
	@cargo world years 24 | tail -1 | sed 's/^ok$$/ok years: twenty-four years, no kind dies out and each settles/'

years: ## the island's creatures by the day for twenty-four years: each kind's numbers year by year, whether each lasts and settles, and who hunted what fell
	@cargo world years 24

sound: ## no window: five minutes heard in a meadow by day and by a shore from dusk, judged from the trace: how many files, how loud against a footfall (tools/audio/listen.sh forest 21 for any place and hour)
	@cargo build -q --release -p cube_game
	@tools/audio/listen.sh meadow 10
	@tools/audio/listen.sh shore 18 10 --busy 100

seasons: ## no window: the meadow in summer and winter, judged by the ground's colour: green, then snow
	@tools/seasons.sh

wind: ## no window: a wood pictured half a second apart in still air and in a hard wind: the leaves must move in the one and not the other
	@tools/wind.sh

blow: ## no window: shots/wind.gif, four seconds of the meadow in a strong wind, for a person to watch (WIND=0.3 for a light one)
	@cargo sandbox --scene shots/wind.gif --play --script '0.1 stop' --gif 4 --big --view three --dist 12 --hour 15 --wind $(or $(WIND),0.85) 2>&1 | grep -q '^shot' && echo "shots/wind.gif"

pace: ## no window: the player run across the world for half a minute on the graphics card: how long the world took to be ready, and every frame's time while he ran, against bars
	@tools/pace.sh --yaw 0.7

swim: ## no window: the player is put in deep water in the real game; passes if he floats, swims and is heard
	@tools/xkeys/swim.sh

window: ## the real window on a screenless display, driven by real key presses: the loading screen holds W off and lifts, then W walks, 2 takes the torch, I opens the pack
	@cargo build -q --release -p cube_game && tools/xkeys/window.sh target/release/cube_game

fight: ## no window: the player fights a goblin in the real game; passes if he is hit, kills it, the sounds played, and he takes its meat from where it lies
	@LOOSE_TRACE=1 cargo sandbox --scene shots/fight.png --play --script '0.5 stop; 1 spawn=goblin; 1.2 use; 2.4 use; 3.6 use; 4.8 use; 6 use; 7.2 use; 9 lift' --after 10.5 --view three --hour 13 2>&1 | awk '/^loose: took [0-9]+ meat/ { meat = $$3 } /^play/ { if (match($$0, /health=[0-9]+/)) { h = substr($$0, RSTART + 7, RLENGTH - 7) + 0; if (h < low || low == "") low = h } if ($$0 ~ /kills=1/) killed = 1 } /^sound swing/ { swing = 1 } /^sound monster_die|^sound goblin/ { voice = 1 } END { ok = (low < 100 && low > 0 && killed && swing && voice && meat > 0); printf "%s fight: lowest health %s, killed %d, swing heard %d, goblin heard %d, meat taken from where it lay %d\n", ok ? "ok" : "FAIL", low, killed, swing, voice, meat; exit !ok }'

sheet: need-id ## no window: shots/ID.png, every clip of one creature (ID=wolf)
	cargo forge sheet $(ID)

moves: need-id ## one creature's movement made and judged: forge fits the walker, bakes its gaits and checks them by number (a line), then one side sheet of walk, run and sprint (ID=wolf)
	@tools/moves.sh $(ID)

person: ## no window: shots/person.gif, a person of boxes moved by their skeleton and feet and no clips (cube_anim::biped): standing, walking, running, sprinting, stopping
	@cargo sandbox --scene shots/person.gif --limbs-person --gif 16 --big --hour 13 2>&1 | grep -q '^gif' && echo "shots/person.gif"

forget: ## no window: a tree felled and cut up in the real game, its regions dropped and made again from the seed and the journal of edits: no cell may differ
	@tools/forget.sh

dig: ## no window: the ground dug with the shovel and the earth put back in the real game, the region dropped and made again from seed and journal: no cell may differ
	@tools/dig.sh

build: ## no window: a tree felled, limbed, broken and split into hunks in the real game, four taken up one at a time and put down each on the last; the region made again is the same
	@tools/build.sh

trees: ## no window: a tree limbed, bucked and split in the real game at four places, two of them great trees; nothing of it comes loose unseen and no tick over 60 ms
	@tools/trees.sh

stone: ## no window: a rock broken with a pick into hunks in the real game, one taken up and put down; blows that do nothing are not kept; the region made again is the same
	@tools/stone.sh

fells: ## no window: the tree nearest each of 25 places felled with an axe: each stands to its last blow, comes loose, goes over and lies on the ground
	@tools/fells.sh

digpace: ## no window: sixty holes dug on the move in the real game, every frame timed against the bars a frame is held to
	@tools/digpace.sh

shatter: ## no window: a stone let go five metres up in the real game comes down, breaks into lumps that lie where they fall and is heard; the region made again is the same
	@tools/shatter.sh

limbs: ## no window: shots/limbs.gif, a body of boxes moved by its skeleton and feet and no clips (cube_anim): standing, walking, running, lying down, getting up
	@cargo sandbox --scene shots/limbs.gif --limbs --gif 16 --dist 6.4 --pitch 0.55 --hour 13 2>&1 | grep -q '^gif' && echo "shots/limbs.gif"

beside: ## no window: shots/beside.gif, the library's own figure and ours (ID=player) through the same keys, side by side
	@tools/beside.sh $(or $(ID),player)

features: ## every creature in the world held to assets/creatures/requirements.toml, and which of its clips nobody has looked at (shots/features.md)
	@python3 tools/features.py

legs: ## no window: the player walked up and down a hillside and stood on a ledge: how far under the drawn ground any knee or ankle went
	@tools/legs.sh

feet: ## no window: the player and a deer walked over a hillside with feet as the clip has them and then put on the ground: how far the standing foot strays each way (tools/feet.sh ID X,Z YAW for another)
	@tools/feet.sh player -60,20 2.0
	@tools/feet.sh deer 30,-40 0.5

cave: ## no window: shots/cave.png, light under the ground on one sheet: a cave dug into a hillside (--cave), by day, with the sun down the hole in its roof, by torch, and at night by a torch and by a fire; look after any change to exposure, the sky's fill or the torch
	@tools/cave.sh

sky: ## no window: shots/sky.png, the sky and the light at twenty fixed hours and places (dawn to full night, the clouds, the torch, a wood, a lake), one sheet; look after any change to sky, sun, moon, cloud, exposure or a light (tools/sky.sh dusk for one)
	@tools/sky.sh

held: ## no window: shots/held.png, the sword and the torch in the player's hand, standing, walking and striking
	@tools/held.sh

gaits: need-id ## no window: shots/gaits.png, walk, run and sprint of the creatures in ID from the side, large (ID="wolf deer")
	@tools/gaits.sh $(ID)

grown: ## no window: shots/grown/*.png, every kind of tree and rock alone, whole and close (tools/grown.sh oak for one)
	@tools/grown.sh

floors: ## no window: shots/floors.png, the ground of a trail, forest, meadow and shore in one sheet
	@tools/floors.sh

gif: need-id ## no window: shots/ID.gif, creature ID moving through every gait, a jump, a blow and a flinch (ID=wolf)
	cargo sandbox --scene shots/$(ID).gif --play --as $(ID) --gif 12 --view three

status: ## every creature: Tripo steps done, credits, model
	cargo forge status

creature: need-id ## build what creature ID is missing; spends Tripo credits once (ID=fox)
	cargo forge build $(ID)

finish: ## remake every model's clips and checks, free, after changing clips.rs or style.toml
	cargo forge finish --all

test: ## unit tests
	cargo test --workspace

lint: ## clippy, warnings as errors
	cargo clippy --workspace -- -D warnings

cross: ## type-check everything for Windows too (no Windows linker needed)
	cargo check --workspace --target $(WINDOWS)

check: test lint cross ## all three; must pass before a commit

.PHONY: need-id
need-id:
	@test -n "$(ID)" || { echo "give a creature: make $(firstword $(MAKECMDGOALS)) ID=wolf"; exit 2; }
