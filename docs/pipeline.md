# Making creatures

`cargo forge` takes a creature from a line of text to an animated model in the game. The paid steps run once; everything after them is free, local and repeatable. Evidence for the design: [research/2026-11-09-model-pipeline-prior-art.md](research/2026-11-09-model-pipeline-prior-art.md).

## A new creature

```bash
cargo forge new fox --kind wildlife --plan quadruped --height 0.45 "a red fox with a bushy white-tipped tail"
cargo forge build fox --to concept     # 2 credits: draws assets/creatures/fox/concept.jpg
cargo forge concepts fox player        # shots/concepts.jpg: look at it beside the player
cargo forge build fox                  # 65 credits: model, rig, export; then finish and check
cargo forge sheet fox                  # shots/fox.png: look once
```

`build` prints one line when the creature is good, and the failing checks when it is not. It exits 1 on a failed check and 2 on an error.

- **The subject** says what the creature is, with at most one defining feature and two named colours. No style words: the style comes from `assets/creatures/style.toml` ([art-guide.md](art-guide.md)). `cargo forge prompt ID` shows the whole prompt.
- **The height** is in metres to the top of the rest pose (antlers and ears count).
- **How it lives** is a `[life]` table added to `creature.toml` by hand: `temper` (`timid`, `hunter`, `wary`, `hostile`; see [engine.md](engine.md#what-lives-in-it)), `habitat` (`meadow`, `forest`, `rock`, `shore`, `snow`), `group` (how many keep together) and `groups` (how many such groups a world starts with). Without it the creature can be spawned in the sandbox but no world has any. `cargo world run` shows what the new creature does.
- **A bad concept** (wrong pose, joined legs, props): delete `concept` from `steps` in `assets/creatures/ID/forge.lock`, adjust the subject, build `--to concept` again. Each try is 2 credits; a model is 45, so look first.

## What a build does

| Step | Where | Cost | What happens |
|---|---|---|---|
| concept | Tripo | 2 | An image from "subject, plan pose, style" |
| model | Tripo | 45 | A textured mesh from the image, 8,000 triangles, colour only |
| rig | Tripo | 20 | Skeleton and skin weights: a biped on the Mixamo-named skeleton, anything else as Tripo reads its shape |
| export | Tripo | 0 | Tripo's preset clips put on, the file saved to `.work/ID/rigged.glb` |
| finish | local | 0 | Reads the skeleton's roles, stands the model at the origin facing +Z at its height, names Tripo's clips, makes the rest, measures speeds, writes `model.glb` and `meta.toml` |
| check | local | 0 | Holds the model to the bars in `style.toml` `[checks]` |

`forge.lock` records the Tripo project and the steps done, so a build run again spends nothing. `.work/` is scratch: if `rigged.glb` is gone, `build` fetches it again for free.

## Body plans

A plan (`[plans.*]` in `style.toml`) says how the concept is posed, how Tripo rigs it, which of Tripo's presets to take, and which clips every creature of the plan must have.

| Plan | Rig | From Tripo | Made by forge |
|---|---|---|---|
| `biped` | humanoid, Mixamo names | idle, walk, run, attack (slash) | sprint, jump, fall, land, swim, hurt, die; and walk or run where Tripo's slides |
| `quadruped` | as Tripo reads it | nothing | every clip |

A plan can also cut a preset that runs long (`trim`: clip → from, to, speed): the biped's `attack` is 2.4 s of Tripo's 6.6 s overhead chop at double speed. Tripo's biped `hurt` is a figure sat wounded for 14 s and its `jump` is three hops on the spot, so ours are made. Its `fall`, `dive` and `climb` presets are free and have not been looked at.

Tripo's one animal clip, a walk, is not used: measured, its forefeet never reach the ground and the feet that do slide at 90% to 180% of the body's speed. `made = ["attack"]` in a `creature.toml` makes a clip for that creature where its plan would take the preset (the troll: the overhead chop tore its skin at 94%).

### What flies is rigged here, not at Tripo (2026-11-11)

Tripo's rig of a bird is no use: the crow (67 credits) came back with a leg named as spine and a wing as tail, its wings folded where no bone could open them, and seven of its ten clips fail. So there is a plan `flyer` (`style.toml`): the concept is drawn from above with the wings spread flat, the model is made and exported and **not rigged at Tripo** (20 credits saved), and `forge finish` gives it its bones from its shape (`crates/forge/src/shaped.rs`): a body, a fore and an aft end, two bones along each wing, every point held by where it lies along the span and the body. The bones are ours, so the clips (`fly`, a wingbeat with the outer wing a little behind; `glide`) find them by name. A flyer's size is its `span` in `creature.toml`.

Tried end to end on the old crow's mesh with its skeleton taken off: rig, skin, both clips, checks and sheet all run. That mesh stands with folded wings, so it twists (its widest way is beak to tail); the path wants a model made for it. **Not yet done: any such model.** The hawk is set to this plan with its lock cleared (its concept was drawn standing), and the account had no credits the day this was written: `cargo forge build hawk --to concept`, look, then `cargo forge build hawk`, about 47 credits. Then the air's birds (`cube_game::air`, still shapes made and moved in code) can be drawn from it. Butterflies, dragonflies and fish can take the same rig with their own clips.

### A four-legged animal on our own skeleton (`plan = "walked"`)

On it: wolf, bear, fox, moose, boar-beast, goat, lynx, deer, badger, pony (the pony is built but not in the world: `groups = 0`, the developer's call). Not yet, each tried: beaver (flat tail; lying far off: chest 59% of a leg up), squirrel (hind legs not found on the mesh; back 15° lying). Never tried: frogs, tortoise, lizard, hedgehog (not built like a dog), rabbit (it hops; the walker has no hop). One whose `rests` is `stands` (pony, moose, lynx) is not laid down: its `lie` is its standing.

Tripo makes the mesh; forge makes the skeleton, the skin and the motion (`crates/forge/src/own.rs`). Use this plan for a new four-legged animal. To move an existing one to it: set `plan = "walked"` in its `creature.toml` and run `cargo forge finish ID`. No credits.

What `finish` does, in order:
1. Takes off whatever skeleton the file has. Keeps the mesh.
2. Measures the mesh: floor, feet, belly, back, which end is the head (the higher end).
3. Finds each leg's joints: takes points across the mesh's faces (not its corners alone: a bear's legs have few) that are over that leg's foot (within 28% of the way to the other pair: a fox's tail hangs behind its hind legs), cuts them into 10 slices from floor to belly and takes each slice's middle. Paw: lowest slice. Elbow: where a fore leg meets the chest. Hock: the point furthest back in the lower half of a hind leg. Knee: the point furthest forward above the hock. Shoulder and hip are inside the body: placed above the elbow and knee by fixed ratios. `FORGE_TRACE=1` prints what was found; a leg not found gets a dog's proportions.
4. Places 21 bones on those joints (trunk, neck, head, four tail, 3 per fore leg, 4 per hind leg) and binds each skin point to its nearest bones. A leg's bones hold only what is over that leg's foot and on its own side; what hangs on the body's middle line behind the hind feet is the tail's.
5. Makes `idle`, `walk`, `run`, `sprint`, `lie` by running the four-legged walker (`cube_anim::Walker::shaped`) on those joints and recording every bone. Then, over each stride: the head is held level against the trunk's rocking and the tail swings side to side, its end after its root (`Own::lively`).
6. Makes the other clips the way every animal's are made (`attack`, `hurt`, `die` carried from the hand-animated wolf; `jump`, `fall`, `land`, `swim`, `eat` by recipe).

Then check, in this order:
- `clip idle ... stretch` must be near 0%: standing must equal the mesh as made. If not, step 3 misread a joint.
- `cargo sandbox --shot ID --view three --clips idle,walk,sprint,attack,die,eat,lie --cols 6`: look once. Use `three`, not `side`: from the side the far legs hide behind the near ones.
- `die` must end on the ground, `eat` must lower the head. If either goes the wrong way, the side axis is wrong (`Own::roles`: side is up crossed with ahead; a leg on that side is the left).

Known faults: skin pinches at elbows and tail root (bound by distance only; smoothing the weights over the mesh was tried and made stretch worse, see `own.rs`); its walk is a trot at walking speed (diagonal pairs), where a real walk sets down one foot at a time.

Lying is held to `assets/creatures/poses.toml` (`pose lie` in the check). On this skeleton the measure uses the known joints (second bone of a leg is the elbow or knee, a hind leg's third is the hock), and a joint counts as inside the body only if it is nearer the body's middle plane than when standing. Guessed as on a Tripo skeleton, the hock was taken for the knee and a correct pose was refused. How high the trunk lies and how far the tail is lifted clear (or let back down to hang) are fitted per body: `Own::clip_judged` tries 90 combinations and keeps the one the measure likes best; a pose that tears the skin past the plan's `max_stretch` is not a pass.

Every animal on this plan lies the same way (on its chest, forelegs ahead). `rests` in `creature.toml` is not used by it: a fox curled and a bedded deer are not built.

In carried clips (`attack`, `hurt`, `die`) the neck and head get half the turn and the tail a third: our two bones stand for the hand-animated wolf's four and eight. `FORGE_TRACE=1` prints, for each clip, which bones hold the most-stretched skin.

### A person moved by the two-legged walker (`walked = [..]` in `creature.toml`; the player)

Tripo's humanoid skeleton and skin are kept (they are sound). Motion is not from clips: the game runs the two-legged walker (`cube_anim::biped`) every frame and turns each bone to it (`cube_game::stride`).

To put a humanoid on it: add `walked = ["walk", "run", "sprint"]` to its `creature.toml`, run `cargo forge finish ID`. Forge then writes into `meta.toml`:
- `[stride]`: every bone's resting turn. The game stands the skeleton this way each frame before the walker poses it.
- `[grip.Left]`, `[grip.Right]`: where a handle sits in the palm and each finger bone's turn when closed (`crates/forge/src/grip.rs`). Written for every humanoid.
- `walk`, `run`, `sprint` clips recorded from the walker (`strider.rs`): used for sheets and for the game's speeds only.

In the game the walker does: standing (breathing, weight shift), any speed from walk to sprint, turning, stopping, slopes and steps, jump, fall, landing, a stance for a blow. Clips still do: the blow and the flinch from the chest up (over the walker's legs), death and swimming (whole body). `--clips-only` turns the walker off to compare.

Check after any change to the walker or the model:
- `cargo test -p cube_anim` (gait, slope, jump and stop are tests).
- `make person`: the same walker on a figure of boxes. Fix motion here first, not on the model.
- `make legs` and `make feet`: the model on a hillside; nothing under the ground, the standing foot within a few cm of it. `STRIDE_TRACE=1` on any `--scene --play` prints each frame a knee or ankle is under the ground.
- `make held` and `cargo sandbox --scene X.png --play --script '0.1 stop hold1; 1.6 fwd walk' --strip 3 --view hand` (`lefthand` for the bow): the thing in the closed hand, close.

What is in a hand (`assets/world/held.toml`): `turn`, `shift` from the grip; `aim` which way it points; `droop` leaves the arm to the walker and only keeps the forearm low (sword, bow); `arm` holds the whole arm in a pose (torch, shovel).

**The walked animals are held to what they can do** (2026-11-14). `requirements.toml` named the plans `biped`, `quadruped` and `bird`; nearly every four-legged animal has been on `walked` since 2026-11-13, so fifteen of them were held only to standing, moving, striking and dying. They are now held to sprinting, leaping, swimming, feeding and resting as well, and the crawlers to leaping: all had the clips (the bullfrog, toad and tortoise are `without` a sprint), and the 33 of those clips nobody had looked at were looked at on side sheets and are in `seen.toml`. A moose and a lynx rested standing (`rests = "stands"`, so their `lie` was a stand); on the walked plan both lie down well, on the belly with the legs folded, and do (`rests = "belly"`, looked at on a side sheet).

### What crawls is built here, of boxes (`[built] form = "crawler"`, 2026-11-13)

For anything on six or eight legs (`crates/forge/src/crawl.rs`): beetle, grasshopper, crab, spider, scorpion. Free; no Tripo. In its `creature.toml`:

```toml
[built]
form = "crawler"
length = 0.22      # body, metres
span = 0.34        # foot to foot across it
tail = 0.55        # abdomen, of the body's length
legs = 8           # default 6
back = [..]  belly = [..]  wing = [..]  mark = [..]   # body, underside, legs, head
```

Then `cargo forge finish ID` and `cargo sandbox --shot ID --view three --clips idle,walk,attack,die --cols 6`. Forge makes the boxes, a skeleton (body, head, abdomen, three bones a leg: hip, knee, and a foot that holds nothing and is what standing and sliding are judged by), and every clip of the plan: legs step in two alternating sets, the set on the ground going back at the body's speed. Each box is held whole by one bone, so stretch is always 0%. A leg rises from the body's side to a knee over its back, half way out, and comes down to its foot at full reach, each part a run of small cubes along its line (out level and straight down it was a table's leg; one box a part was a slab).

Its own parts, each a line in `[built]`: `claws = true` (pincers held out ahead: crab, scorpion), `sting = true` (a tail arched over its back, on five bones of its own after the legs', so it can straighten: scorpion), `leaps = true` (long hind legs folded high: grasshopper; it still walks, it does not leap).

**A snake** is `form = "serpent"` (the adder, 2026-11-14): a head with a tongue and fourteen lengths that taper to a tail, each a bone on from the last, banded in `back` and a darker `back`, `belly` under; `length` is all there is to give. It has no legs and no gait: every clip is the one wave (`Crawler::coiled`) run down the body, wider and faster to go, drawn straight to strike, and it dies rolled onto its back. From any side it is a line on the ground, so it is looked at from above: `cargo sandbox --shot adder --view top`. Forge's `roles` check asks a serpent for a head and ten lengths, not for legs.

**A bird on the ground** is `form = "fowl"` (the same file; heron, duck, pheasant, owl, hawk, crow, all in the world): `length` its body, `span` how long its legs are, `chord` its neck (of the body's length), `tail` its tail, `mark` its beak and feet. Two legs under it of three bones (thigh, shank, and a foot that stays flat); it steps at a bird's rate (two steps in 0.5 s walking, 0.3 s running) and goes as far as its legs take it; it pecks with its head, not its body. Each wing is two slabs folded down its side on two bones (`wing_l`, `wing_l_tip`, after the legs), opened and beaten by `Crawl::wings`: its `jump` is the take-off, its `fall` is flight (three beats a second), its `land` folds them. `flies = true` in `[life]` has the simulation fly it (engine.md, What lives in it). All six are in the world. Spider and scorpion at a hunter's numbers (`share = 0.1`, `range = 260`, `capacity = 4`): at the common share the goblins died out within sixteen years (`cargo world years 24`).

### The small life of air and water is built here, of boxes (2026-11-11)

`[built]` in a `creature.toml` (`crates/forge/src/built.rs`): a form (`bird`, `insect`, `fish`), a length, a span and four colours, and `forge finish` makes the model itself, of boxes at its true size, then rigs it with the rig above and makes its clips. No Tripo, no credits, and it is the world's own style. Six are made: `songbird`, `swallow`, `soarer` (the hawk overhead), `flutterby`, `darter` (a dragonfly), `minnow`. Plans: `flyer` (`fly`, `glide`, `perch` with the wings folded), `flutterer` (`flutter`, `glide`), `swimmer` (`swim`: a wave down the body, the tail widest). All pass `forge check`; `cargo forge sheet soarer` shows the wingbeat.

The game draws them as `creatures::Picture`: a model and its clips with no body in the simulation, moved by `cube_game::air` as before (the paths, the flocking, the schools are unchanged). Until this day each was a shape made in `air.rs` with wings turned by hand, outside the pipeline.

## Where motion comes from

**A person's motion comes from a person: a hand-animated library for all but the leap (Tripo's, from a description), used as recorded; a clip is made here only where neither has one. What open work there is on models and motion, and why how clips are played is now the weak part: [research](research/2026-11-12-open-animation.md). An animal's motion is a hand-animated wolf's, put on its own body by its feet.** (The developer, 2026-11-11: "if Tripo animates well we should rely on it.") Every time our code rewrote a recorded clip it got worse while every check passed.

| Clip | Bipeds | Four legs |
|---|---|---|
| idle | the library (`Idle_Loop`) | the library's wolf (`Idle`) |
| walk | the library (`Walk_Loop`) | walked (`walker.rs`) |
| run | the library (`Jog_Fwd_Loop`) | walked (`walker.rs`) |
| jump, fall, land | the library (`Jump_Start` from where the feet leave, `Jump_Loop`, `Jump_Land` played twice as fast) | made |
| attack, sprint, hurt, die | the library (`Sword_Attack`, `Sprint_Loop`, `Hit_Chest`, `Death01`) | the library's wolf (`Attack`, `Gallop`, `Idle_HitReact_Left`, `Death`) |
| swim | the library (`Swim_Fwd_Loop`) | made |
| eat | none | made |

- **The library** (2026-11-12, after the developer called the sword swing stupid and asked for a new technique): Quaternius's Universal Animation Library, CC0, 43 clips a person animated on one rig. `tools/library.sh` fetches it to `.work/library/` (7.6 MB, not committed); `[plans.biped.library]` in style.toml names the clips taken; `crates/forge/src/library.rs` carries one onto a Tripo humanoid. Nothing is carried in a bone's own frame, since the rigs share nothing but what they are a rig of: for each bone, in the world, how far theirs has turned from the library's standing pose is put on ours, after ours is turned to point as theirs does standing (theirs rests arms down, ours in a T); the hips go where theirs go, by the two figures' heights. A swimmer is raised to where the game holds one; any pose is raised until no skin is under the floor (a troll laid where the slight figure lay was in the ground); the sprint is played slower to be half as fast again as the run. It held on all six bipeds at the first try, goblin to golem. More of its clips are there for the taking (roll, torch idle, punches, crouch, pick up, sit).
- **Animals take a hand-animated wolf's motion** (2026-11-12; Quaternius's Ultimate Animated Animals, CC0, `.work/library/Wolf.glb` from https://poly.pizza/m/P1gU3Qkr9r; `[plans.quadruped.library]`; `Library::carry_beast`). Tripo's animal rigs share no names, so it goes by roles: trunk, neck and tail bones turn as the wolf's bone the same way along its chain; each leg's line from hip to foot lies as the wolf's leg lies, by the two legs' lengths, and the leg is bent to reach it. (Feet put where the wolf's stood on the ground, whatever the body did, left a dead wolf on its back with its legs in the air.) A walk is set down on its lowest foot moment by moment and a gallop by the stride. Speeds stay each animal's own (what its made gait would go at): the walk is played faster or slower until its feet hold, so a badger steps quicker than a wolf; the gallop is not, since its feet say less than it covers. Not carried: onto a body whose made clips are eased (`ease` under 1: fox, toad and bullfrog tore at two to three times the bar), nor a gait onto what hops. Eleven animals take it. Seen on one sheet beside the made clips: the walk lifts its paws, the gallop gathers and reaches, the blow is a lunge, the death a fall onto its side.
- **Tripo's own idle and walk lean back** (the developer, 2026-11-12: "the player's shoulders lean back really far"; a side sheet shows the chest behind the hips in both). Standing, walking and running are the library's too now, which stand upright. They go at the speed Tripo's own went (measured as it was used), each played until its feet hold: every creature's speeds are as they were.
- **A carried clip is played at its own rate, and each gait's true speed is written beside it** (`[clips.walk] speed` in meta.toml, 2026-11-12). The library's jog is a 5.4 m/s run and its sprint 8.3 m/s (read from its root-motion file, `Library::travels`: read off the feet they said 4.8 and 7.1). They had been played at seven tenths of their rate to go the game's 3.6 and 5.4: a run in slow motion on sliding feet, which the developer called not natural whatsoever. The game mixes gaits in the share that covers the ground at the speed being gone, so the stride shortens and nothing is slowed.
- **The library's own figure is a creature** (`assets/creatures/mannequin`, plan `authored`, not in the world): its clips as they were made, at their own speeds. `make beside` records it and one of ours through the same keys, side by side. Where the two move differently the carrying is at fault; where both look wrong, how the game plays clips is. Look at this before changing either. Set pose by pose on one sheet (`cargo sandbox --shot mannequin --view side --clips run,walk --cols 8`, and the same of the player), the carried run and walk are the library's own to the eye, from the side and the front; A person goes at its clips' own speeds since (the player walks 0.95 m/s and runs 5.2, where it was 1.13 and 3.6; sprint is a quarter over the run): at the game's old speeds the run was a mix of walk and run and looked the worse beside the library's figure; at the clips' the developer called the two about equal.
- **The routine for what goes on four legs** (2026-11-12; `make moves ID=wolf`: one line and one picture). Its walk, run and sprint are not drawn or carried: forge walks the model. `walker.rs` fits `cube_anim`'s walker to the model's own legs (where each leaves the body, where it folds, how long its bones are), walks it at its true size and at the speed the gait is for with no window, and keeps one stride as the clip (source `walker`); a trot becomes a gallop by the legs' length. The baked gait is kept only if it does not tear the skin; then every check there already was judges it (feet on the floor, no slide, loop closed, stretch), and the game plays it as any clip. Of nineteen: twelve walk so; toad, bullfrog, tortoise and lizard keep their made gaits (legs out sideways: torn), moose and hedgehog theirs (legs of two bones), the rabbit its hop. A leg is put down by the point of it that meets the ground (the floor under its last joint as it stands, carried by that joint: `sole`), not by the joint: Tripo's rigs are not even (the wolf's off hind leg has a bone fewer than its near one and ends at the hock), and put down by its last joint that leg stood on its hock in the air. A new animal needs nothing said: build it, `make moves ID=…`, read the line, look at the sheet once, add what was looked at to `seen.toml`. Change the walker itself in `cube_anim`, with its tests, and look at it on the boxes first (`make limbs`). Not in the routine yet: resting, feeding, a blow and a fall are still clips made or carried; two legs.
- **A pose is built to what was seen, and measured against it** (`assets/creatures/poses.toml`, 2026-11-12; the developer: find a process that lets you understand what the animation should look like, a defined process to build it, and ways to improve it as we go). Five steps, written at the head of that file: look at a dozen photographs of the real animal at once on one small sheet; write what makes the pose as things measurable on a skeleton; build it in forge to those; forge measures what it built and says each in a line (`pose lie …` in `cargo forge check`, `FORGE_TRACE=1` while building) and keeps the clip only if all hold; look at one sheet, and where the eye catches what the numbers passed, add the measure and write the lesson under `learned`. Lying was the first: three tries from imagination were wrong; from photographs it is chest and loins on the ground, forearms flat ahead, hind legs in a Z with the lower leg flat from hock to paw. Two measures were added by looking (chest and loins apart; the tail). The pose is fitted as a whole (`walker::lie`): the model's thickness is read off its skin round each joint, the pose is seven numbers (body height and tilt, where the fore paws and the hocks go, how high the limbs lie), one number says how far a pose is from every measure at once on the skin as drawn, and the seven are searched together from several first guesses. Placed part by part it could not be done: clear the ground and the legs went into the body, mend that and the chest rose. Bear and boar-beast pass and lie so; the wolf comes near (rump up) and is not kept; goat, badger and squirrel are further off, each with its numbers.
- **To a recorded clip forge does only this:** closes its loop, stands it over the creature's own place and floor by one shift (Tripo's run sits most of a metre ahead of its walk), trims it (`[plans.x.trim]`). A leap is stood on its own lowest foot frame by frame, since the game lifts the body.
- **A recorded gait's speed is read, not set:** the middle speed of whichever foot of a pair is lower, checked against how far the steps reach (the steps are believed where the two differ by over a quarter). `meta.toml` speeds are the speeds the game moves at.
- **A motion from a description is an asset at Tripo:** 20 credits once, then free on any humanoid. `tripo.ts animate PROJECT --prompt "…" --seconds 4` makes one and reports its asset id; `[plans.biped.motions]` lists the ids and every biped's export carries them. Judge it before adopting, on a sheet beside what it would replace: single actions come back well (the jump); cycles and lying poses do not (a sprint came back as a jog getting up speed, a breaststroke and a front crawl lying nearly still, a sword slash as a weak turn): those are the library's now. A held pose comes back well too: `carry` ("a man stands still holding a heavy box", 2026-11-13) is arms out before him at the waist, played from the chest up over the walker's legs while a hunk is carried (`stride`, `motion`); the library has no carrying (its `Push_Loop` is a man leaning into a wall). Only the player has it: the monsters carry nothing (the developer), and each says `without = ["carry"]`; it goes on any of them for nothing if they ever build. `tripo.ts show PROJECT` gives each described motion's asset id. Seven motions tried, 140 credits.
- **Tripo's presets that are no use:** biped `jump` (skipping on the spot), `hurt` (sitting wounded); `quadruped:walk` (marches on the spot, 0.14 to 0.27 m/s on a wolf). `dive` (a roll) and `climb` (a vault) are good and unused.
- **Made four-legged gaits** (now the trot of every animal, and every gait of what hops or tears)**:** each foot is carried straight back at the gait's speed while down and swung forward in an arc while up, the leg bent to reach it; speed from leg length by Froude number. The spine sways, the head nods, a gallop's back rocks. `hops = true` (the rabbit) lands hind feet together, then fore.
- **Made grazing (`eat`) aims the muzzle at the ground, and the muzzle is read from the skin** (2026-11-11): no bone ends at a muzzle and the head role is unreliable (a goat's one head bone carries only its horns), so `check::nose` takes the skin's corner furthest forward and low. The neck is whatever of spine and head lies ahead of the shoulders or just behind; it swings down from its root, and past that the body tips forward about the hips with the fore legs giving. A bone well behind the shoulders turns 0.45 rad at most (a bear folded in half), and a body on legs with no knee does not tip (a moose's feet went through the floor). Before, one bone took a fixed dip: a pony's neck rolled up under its chest and a goat's hardly moved. Judge on one sheet of several animals from the side (`cargo sandbox --shot ID --view side --clips eat`).
- **Tried and dropped:** locking a recorded gait's feet to a line and stretching its stride; replacing a recorded walk with a made one; putting CMU capture on Tripo's skeleton in Blender by turns from rest (twisted trunk: the rests were not the same pose, which the library's carrying allows for); Tripo's `slash` for a blow (a slow two-handed chop that bowed the whole body), with the arm then posed over it from `held.toml` (the two fought). A hand-animated CC0 animal (Quaternius) moves better than our made gaits and looks plainer: [research](research/2026-11-11-animal-animation.md).

## What motion has to be, before any of it is tuned

Written 2026-11-14, after a day of softening the player's code-made body and the developer
still seeing it stiff: "it sounds like we're just missing some fundamentals to how animations
should behave or be designed". These are the fundamentals; a fault in motion is first asked
which of them it breaks, and only then tuned.

1. **One performance moves the whole body.** A body reads as alive when its parts answer one
   another: the shoulders to the hips, the arms to the legs, the head to the trunk. A recorded
   or hand-animated clip has that in it. A body whose legs are one model and whose arms and
   chest are formulas beside it does not, however the formulas are softened.
2. **Code corrects a performance; it does not replace it.** What code is for: feet onto the
   ground that is there, the hips let down for them, a lean into speeding up and into a turn,
   a look toward something, a held thing. Each is a layer added over the clip, small, eased,
   and able to be turned off to see the clip alone.
3. **Nothing changes at a stroke.** Every value that is drawn (a height, a turn, a weight
   between two clips) comes to its new value over time. A rule that holds at a stroke (a leg
   no longer than it is, a body out of the ground) is met by what feeds it, not by a jump.
4. **A gait is paced by the ground.** The stride's place in its cycle goes forward by
   distance covered, not by time, and gaits that are mixed share that one place, so a foot
   that is down is still and two clips never fight.
5. **What is hung from the body comes after it.** Arms, a tail, ears, a held sword trail what
   carries them and settle after it stops: overlap and follow-through.
6. **It is judged moving, by numbers that mean what the eye sees.** How fast the trunk rises
   and how hard it jolts (`trunk` in a scripted scene's lines), whether a standing foot
   slides, whether a limb is in the ground; then one film, not stills.

Where we stand against them: animals keep 1 (a hand-animated wolf's motion carried by the
feet) and break 3 at a run on blocks. The player breaks 1 while he moves: his body is the
walker's legs and formulas above them (`stride`), which is why his shoulders do not answer
his legs. `--clips-only` is the other way round, the library's recorded walk and run with
feet put on the ground: `shots/player-compare.mp4` sets the two side by side.

## Judging motion

**A bird pecks with its whole body** (`crawl.rs`, `eat`, 2026-11-13): it tips forward over its feet, which stay put, as far as brings its beak to the ground with its neck down, by its own leg, body and neck. By its own boxes, to the centimetre, and only up from the ground and back: by a rule of thumb a duck's beak went into the ground and a heron's stopped short. A blow or a flinch made for an animal on our own skeleton is raised, any moment its lowest skin would be under the floor, as what is built of boxes is (`finish.rs`). With only the neck turned, every bird's beak pointed down at the height of its back. Found by looking at all six from the side on one sheet; the other clips of everything built of boxes, and jump, fall, land and idle of every four-legged animal, were looked at the same day and nothing else was wrong. The hedgehog stood tall on straight legs and the rabbit upright: each now has `low` in its `creature.toml` (how far down on its legs it stands and goes, of 1; `own.rs` bakes its idle and gaits there), the rabbit moved to our own skeleton for it with its hop kept, and the blow, flinch and death of each are `made`, from where it stands, since the wolf's carried over began with it up on straight legs.

**What is dead is limp** (the developer, 2026-11-13: "the model can change shape when its dead so it lays on the ground because dead things dont stay stiff"). A crawler's death (`crawl.rs`): over onto its back, then its legs fold at the knee and fall flat, a sting's tail straightens along the ground, and all of it is let down onto the floor (`finish.rs`: a box-built body is raised wherever it would be under the ground, and in dying also lowered onto it). Stiff, a dead scorpion stood on its curled tail with its body in the air. Make a death by what the body would do with nothing holding it, not by turning the standing pose over. How each kind comes to rest, each looked at: a spider's legs drawn tight in over its belly and crossed (hip -1.6, knee -1.2: four pairs tried), a beetle's and a crab's folded in more loosely, a scorpion's and a grasshopper's fallen flat with the grasshopper's long hind legs folded down over it (curled, they stood under it and held it up); a bird on its side, its head let down on its neck no further than the ground its body lies on (let right down, a heron's neck held its body up), its tail dropped, the wing uppermost sagging a little (let fall open it stood up like a flag). A stiff-skinned animal on its back can splay its legs only 0.15 radians before its skin tears (bullfrog 144% at 0.2): it is on the ground, and still stiff. The four-legged animals' and the people's deaths are recorded motion and already go slack. The lizard, the tortoise and the hedgehog are on our skeleton (`plan = "walked"`) since, and die as they do, over onto a side and slack. For that the rig takes which end is the head from Tripo's own `Head` bone where there is one (`own.rs`): by which end stands higher, a tortoise, whose shell is over its head, was rigged back to front and walked tail first in the ground, and a hedgehog's feet floated. And a body that squats with its legs out beside it has its belly found along its middle. The bullfrog and the toad are on it too, and both hop: what `hops` has its walk and run from the hop's recipe laid on our bones by their roles, not from the walker's steps (`finish.rs`); put on our skeleton without that, the frog walked. The toad tore at every hop (137 to 260%) until the check was asked where (`FORGE_TRACE=1`: `stretch NAME where`, the pairs of bones its worst edges lie between and the box they fill): between its hind paws and its trunk, just behind its hind feet. A squat body's folded hind legs and long feet reach further back than a leg was let hold, and what lay beyond was the trunk's; its hind legs now hold all that is behind them and off its middle line. Two guesses before that (its flank, its toes) changed nothing and were taken out. All five that were stiff die slack on the ground now.

**A stiff-skinned animal dies onto its back** (`clips::make`, `ease < 1`, 2026-11-13): toad, tortoise, bullfrog, lizard, hedgehog. On its side its legs do not fold, and it lay dead propped off the ground on the two under it; the check on a dead body's height passed, since a leg did touch. Seen only on a sheet of two animals at full width (`--shot ID --clips attack,die,eat,lie --view side --cols 6`, two stacked): six moments of a clip at a third of that size had hidden it.

**A bound keeps the belly off the ground** (`cube_anim::Walker`, 2026-11-13): seen on `make gaits ID="wolf bear"`, not by any number: mid-bound a bear's trunk came down to the ground, a stumble in every stride. In a bound a pair of feet now steps when it is a third of a stride from where it belongs (not a half: the body goes as far again while the pair is in the air, and the pair on the ground was left further behind than a leg is long), and the trunk comes down to its feet by a quarter of its height at the most. Wolf, bear, boar and moose looked at after.

**A number that passes is not a clip that looks right.** Two bars failed good motion for weeks: `slide` read a recorded human walk as 88% (an ankle does not move at one speed while its foot is down), and the 60% `stretch` bar had animals' strides cut to as little as a twentieth. One number missed a real fault: the pony's fore hoof pulls into a spike at full stride.

- `make gaits ID="wolf deer"` draws walk, run and sprint from the side, eight moments each, large enough to see a spine and a hoof: `shots/gaits.png`. Look at it after any change to a gait, a plan or a creature's knobs, and set a changed clip beside what it came from.
- `slide` is a bar for made gaits only. `stretch` is 60%, 110% for four legs (full strides read 70% to 103% and hold), 80% for bipeds.
- To see what Tripo offers before using it, fetch the presets untouched into a trial creature and draw them (a plan whose `presets` are the names, a creature dir with the other's `forge.lock`).

- `gait_gain` in `creature.toml` (under 1) shortens stride, lift, rock and arm swing for a body whose skin tears at full reach: troll 0.6, golem 0.75, lizard 0.6, tortoise 0.45. A made gait uses the average of it and 1, so 0 is half a stride and it may go below 0 for less (the bog toad -0.4, the bullfrog -0.6): a number that wants renaming when the gaits are next worked on.
- `ease` (under 1) does the same for the made clips that are not gaits (idle, jump, swim, attack, eat…): every joint's turn from rest, and the root's travel, scaled. Fox 0.4, tortoise 0.3, toad 0.35, bullfrog 0.3, lizard 0.7. Not `die`, which has to finish on the floor.
- `without = ["sprint"]` leaves out a clip of the plan a creature would not do, and the checks do not ask for it: the tortoise and the frog-types do not sprint, the stone golem does not swim. The game plays the next thing down.

## The roster

36 creatures besides the player are wanted first (100 in the end). `cargo forge status` lists them. Where they stand, 2026-11-10:

| | Creatures |
|---|---|
| In the world, passing | bear, boar beast, deer, goblin, rabbit, troll (six of the first seven, all rebuilt in the style that is not cute); badger, beaver, bullfrog, fox, goat, golem, hedgehog, lynx, monitor lizard, moose, ogre, pony, squirrel, tortoise; the player, now a castaway |
| In the world, failing | wolf (rebuilt: `run` stretches its skin 70% and `sprint` 77% against 60%, whatever its stride and after a second rig; the leaner model's weights are the cause), shambler (Tripo's idle floats it 11% off the floor; a made idle left it in a T-pose, which is worse), bog toad (`die` stretches 71%) |
| Bought, kept out of the world (`groups = 0`) | crow, crab |
| Concept only | the other five birds and six crawlers |

What the batch of sixteen taught, all now in forge:

- **Tripo's animal rig is sometimes short of a leg.** The moose came back with no front legs, the lynx and goat without a hind one. Rigging again (20 credits) mended the moose and the lynx and not the goat. Forge now refuses a four-legged creature that has not four legs, in a line, where it used to crash.
- **A tail that reaches the floor was a fifth leg** (the fox's brush). Legs come in pairs of a length, so the odd one out is not a leg.
- **Toes were legs** (frog, lizard, toad, bird): a low joint whose branches are all short is a foot, and the leg is the chain down to it. Good for the lizard; the frog and toad are still wrong.
- **A leg must come down**: its tip low and a good drop from its root, so a tail lying on the ground is not one. And a limb that ends at the ground is not an arm.
- **A leg of two bones has no knee**: the reach solve let its bounds cross and crashed.
- **A crash in the clip maker is that creature's failure**, said in a line; `finish --all` goes on to the next.
- **A plate of shell was a head** (the tortoise): a lone joint hung straight off the root is a piece of the body, whatever its height, and is not the head.
- **A knee was a foot** (toad, bullfrog): toes are three joints at most, so a low joint with a shin and a foot below it is not one. Frog toes are long: up to 0.6 of the body's height.
- **Twigs**: Tripo's rigs carry leaf joints sat exactly on a sibling, moving nothing. They are not a fork in a limb and not a chain. Of two leaves on one spot the later is the twig (calling both twigs lost the goat its head).
- **A flat drawing makes a model short of a leg.** The goat's first concept was a flat illustration with its far hind leg faint: three models from it all came back with that leg missing from the rig. A concept redrawn as a solid figure with four plain legs rigged at once. Look at the concept for that before buying.
- **The frog-types pass, and barely move.** Their hind legs are folded in four and the gaits are written for a dog's; at full stride the skin tears 150% to 220%. They are turned right down (below) until there is a hop.
- **The head is still the highest chain**, which on a fox or a rabbit is an ear, and on the squirrel its curled tail. A rule that took the muzzle when the highest chain was one of a pair was tried twice more and backed out: with the true head the fox's `eat` tore 180% to 350% on two different rigs, and the moose's head changed too. Taking the joint the ears grow from instead was tried and made the fox, bear and deer worse, so it was backed out; it wants doing with the eat and attack recipes in hand.

**Birds and crawlers do not work yet.** The crow's skeleton reads as two legs now, with a wing taken for an arm, and its clips still tear by up to 460%: the recipes do not know a body that lies level on two legs. The crab has no legs the reader will take (they go sideways and do not come down), and there is no gait for more than four. No more of either are bought until one of each passes. Nothing flies in the simulation either.

**Stopping a batch loses what is in flight.** Four models were being made at Tripo when a batch was stopped on 2026-11-10: charged (180 credits) and never written to a lock. Let a batch finish, or stop it between creatures.

## Roles and made clips

Tripo's bone names cannot be trusted (`bone_7` beside `tripo::0_Right_Limb_0`; a tail bone named `1_Left_Limb_0`). So `rig.rs` reads roles from shape:

- A **chain** runs from a leaf bone up to the first bone that branches.
- Chains whose tip is near the floor are **legs**, front or hind and left or right by position.
- A chain with one of Tripo's `Head_n` bones in it is the **head**, and one with a `Tail_n` bone is never the head (2026-11-11: by shape alone, the highest chain, a squirrel's brush and a beaver's limb were heads and both ran backwards, and a fox grazed with an ear). Where there is no such name, the highest other chain is the head; one trailing behind the feet is the **tail**; the trunk from the root to the head is the **spine**; the rest (fingers, wings, ears) are **others**.
- A limb that leaves the trunk and ends in a branching hand is an **arm** (shoulder to hand); its fingers are the others.
- **Forward** is toward the head when it sits ahead of the feet, else +Z as Tripo exports an upright figure. `facing = "+x"` in `creature.toml` overrides a wrong guess.

`cargo forge inspect ID` prints the roles. Look at it first when a creature moves wrongly.

Clip recipes (`clips.rs`) are written against roles, so each serves every plan: `idle` (breathing, a look about, tail sway), `sprint` for a biped whose run is Tripo's (that run, quicker and leant forward), `jump` (into an airborne pose and held: one knee up and arms out for a biped, forelegs reaching for an animal), `fall` (that pose, limbs stirring), `swim` (a biped lies forward, kicks and reaches its arms over in turn; an animal holds its head up and paddles), `attack` (a lunge), `hurt` (a flinch), `die` (falls and is settled on the floor each frame), `eat` (neck and head down, nibbling), and `land` (sinks onto bent legs and rises, feet held where they were), and a plain `walk` for a body that is neither four-legged nor upright. They are plain and readable on purpose; they are the place to add craft.

What finish does to every clip, so no creature needs hand fixes:

- A made clip is laid over how the creature **stands** (the first frame of Tripo's idle), not its rest pose, so a biped does not snap to a T-pose.
- A recipe that cares which way a limb really points (arms in `jump`, `fall`, `swim`) gives its turns **as the creature stands** (`Stance` in clips.rs): on a T-pose rig "swing the arm forward" is a different turn at rest than with the arm hanging.
- A looping preset is **closed**: its last fifth eases into its first frame (Tripo's biped run was 19° open).

## Checks


- **Every number of a clip is a number** (`check`, 2026-11-13): a frame with a turn or a place that is not one fails. Such a clip draws nothing at all, and passed every other bar, since nothing compares with it: the hedgehog walked unseen until its sheet was looked at. A leg the made gait cannot solve is now left as it stands (`clips::gait`).
- **Its legs go** (`check`, `legs swing`, 2026-11-13): in walk, run and sprint the foot that goes furthest must swing fore and aft at least 30% of its leg's length. A body carried along on legs that hardly stir passed every other bar: the hedgehog, the frog and the toad did, found only by looking. The toad was 21%; everything else is 36% to 213% (`FORGE_TRACE=1` prints it).
- **What has died rests on the ground** whoever made the clip (`finish`): a made death is moved by how far its last frame is off the floor (`check::off_floor`), eased in.
One line each, `ok` or `FAIL name value (bar)`; passing lines print only when something failed, or with `FORGE_VERBOSE=1`.

| Check | Bar | Catches |
|---|---|---|
| triangles, joints | ≤ 12,000, ≤ 80 | A model too heavy for a crowd |
| roles | ≥ 2 legs and a head | A skeleton the recipes cannot use |
| clips | all the plan's | A preset Tripo did not deliver |
| weights | 0 unweighted vertices | Skin left behind |
| stretch, per clip | 99th-percentile edge stretch ≤ 60% (110% four legs, 80% bipeds) | Bad weights or a recipe that tears the skin: the skin is posed on the CPU through each clip |
| loop gap | ≤ 5° | A looping clip that pops |
| feet off floor | ≤ 8% of height | Feet sinking or floating in idle, walk, run, sprint, eat |
| slide, made gaits only | ≤ 25% of the gait's speed | Feet that skate: how fast a foot moves over the ground while it is down, with the gait played at its speed |

The bars are guesses; see Judging motion for the two that were wrong.

## Looking

`cargo forge sheet ID` renders `shots/ID.png` offscreen: a row a clip, six moments each, a slab under each row to read feet against. `--view side` or `front` turns the creature; `--clips walk,run` limits the rows. One sheet is about 1,500 tokens. Ask of it: does each row read as its name, do feet stay on the slab, does anything tear or fold.

`cargo sandbox --scene shots/scene.png --spawn all` draws the test world with one of each creature in it, offscreen.

In the sandbox (`cargo sandbox --spawn ID`) the panel plays clips, scrubs a held frame, sets speed, sends the creature wandering at its measured walk or run speed (so sliding feet show), and draws skeletons.

## Trying a look

`cargo forge looks` draws the creatures named in `assets/creatures/looks.toml` in each look listed there and lays them out as `shots/looks.jpg`, a row a look under the house style's own row. A look is the text that stands where the house style does in a concept prompt. Only concept images are made, 2 credits each, once: they are kept under `assets/creatures/looks/`. A look is judged for more than how it draws: limbs of natural length and set apart (so the rig and clips fit), a biped in a true T-pose, an animal square on and solid (a flat drawing makes a poor model). The one that wins is copied into `style.toml` by hand, and one creature is built in it and held to the checks before the rest are bought.

**Building a creature in a look on trial:** a `creature.toml` with `look = "painted"` takes that look's style in its prompt, and with `look_of = "player"` its concept is the picture `cargo forge looks` already drew of that creature, so the model is made from the image that was chosen and it is not paid for twice. Then judge it as any creature: `cargo forge check`, and `make gif ID=…` for a person to watch.

Built 2026-11-10 (65 credits each): `player-carved` fails skin stretch in every clip (101% to 126% against a bar of 80%: squared limbs crease at every joint) and `wolf-carved` in five; its flat browns were also the least liked. **Carved is dropped.** `player-painted` passes everything and is the only biped whose recorded walk held its feet well enough to keep. `wolf-painted` passes once its stride is shortened (`gait_gain = 0.55`). Its concept was a three-quarter view and the model came back standing at 45° to the axes, walking sideways: forge now reads a diagonal facing from the way a four-legged body's feet are spread, and measures length along it. House and painted were the two left, and **the house style was chosen** (2026-11-10): it is what the eight creatures are already in, so nothing is bought again. The trial creatures (`player-carved`, `player-painted`, `wolf-carved`, `wolf-painted`, and `adventurer` from the cubes trial) were taken out of `assets/creatures/` afterwards; they are in git history at commit `0b541c3` with their locks, should one be wanted back.

Tried 2026-11-10 on the player, wolf and troll (18 credits): `carved` (squared, chamfered block shapes: the nearest to the world), `flat` (strict flat-shaded low-poly; its wolf came back a flat drawing and its troll out of the T-pose) and `painted` (hand-painted and smooth, as Eco; its troll also out of the T-pose).

**A recorded gait that only just fails**: forge keeps Tripo's walk and run only if the feet hold (slide at most 25%), and otherwise makes its own. The made run is a lumbering one, right for a troll and a jog on a person. Where a recorded gait misses the bar by a little, `slide = 0.32` in `creature.toml` allows it for that creature, with a comment saying what it measured (`FORGE_TRACE=1 cargo forge finish ID` prints each recorded gait's speed and slide). Used once, for the player: his run measured 25.4%.

**Taking an idea**: `cargo forge ideas ID` draws every subject in the creature's `ideas.toml` (2 credits each, once); `cargo forge ideas ID --take NAME` makes one of them what the creature is (its subject into `creature.toml`, its picture as the concept, the lock set back to the concept alone), and `cargo forge build ID` then makes the model from the picture that was chosen. Done by hand the first time (the castaway, 2026-11-10) and by the command the second (the plain man, 2026-11-11).

## Cubes

**Parked 2026-11-10: the game keeps its smooth models** ([engine.md](engine.md#creatures) says why). What follows is kept so the trial need not be repeated.

`cargo forge cubes ID` rebuilds a finished model from 5 cm cubes, the size of a bit of the world, as `assets/creatures/ID-cubes/` with the same skeleton and clips. Its output is not committed. This section is the routine for getting a good one, written after getting several bad ones. Why it is built this way: [research/2026-11-10-cube-characters.md](research/2026-11-10-cube-characters.md).

### The routine

1. **Is it worth converting?** A figure wants to be at least 10 cubes tall (half a metre) with limbs two cubes thick (10 cm). The rabbit is neither. Smaller things are drawn for the grid by hand or left smooth; no setting rescues them.
2. **Write the subject for cubes** before any credits are spent, if the creature is new or being rebuilt. In the subject or the plan's pose: thick straight arms and legs; legs set apart with a clear gap; mitten hands, no separate fingers; big plain areas of one colour; no fine detail (straps, laces, fur tufts, thin horns); hair as one solid mass that leaves the face clear; nothing hanging over the top of the legs. Look at the concept (2 credits) for exactly those things before paying for a model (65).
3. **Run it:** `cargo forge cubes ID`. One line, `ok` or the failing checks; `FORGE_VERBOSE=1` shows every number. It exits 1 on a failed check.
4. **Fix by the table below, not by eye.** Change the code or the source, run again.
5. **Then look once:** `cargo sandbox --scene shots/ID.png --spawn ID-cubes --dist 4 --yaw 0.8` (a lone creature is framed, stood on the ground, at about the distance it is seen in play), and `cargo forge sheet ID-cubes` for the clips. Judge it at that distance: a close-up makes any cube figure look crude.
6. **A defect seen that no check caught becomes a check** in `judge` (cubes.rs) with a bar in `style.toml` `[cubes]`, before it is fixed.

| FAIL | It means | Do |
|---|---|---|
| `tall` | Too small to be made of 5 cm cubes | Leave it smooth, or draw it for the grid |
| `triangles` | Too many faces: a big body | Bigger cubes for it (`--size 0.1`), until flat faces are merged |
| `legs-apart` | Two legs touch somewhere | The source stands with its legs together or crossed: ask for legs apart; check the idle's first frame |
| `limbs` | An arm or leg is one cube thick | Ask for thicker limbs; a deer's shins cannot be saved by settings |
| `loose` | Fingers, wisps of hair, crumbs of cloth | Ask for mitten hands and solid hair; it is the source's fine detail |
| `speckle` | One-cube spots of colour | The texture is busy: ask for plain colour areas |
| `face` | No room for eyes, nose and mouth | A bigger head (`--head 1.4`), or hair that leaves the face clear |

| `limbs-shown` | Most of an arm or leg piece is inside another part | A clip made for a slim figure sinks a stout one's arms in its sides; arms are moved out of the trunk, so what is left is a limb folded into itself |
| `face-framed` | The face is the colour of the hair round it | The skin colour was read off hair: look at where the hair falls |

**The first model made to these rules** (`adventurer`, 67 credits, 2026-11-10): the concept came back as asked but for fingers where mittens were wanted. Stood still and walking it is the first cube figure that reads right, with a clear face. It was also the one that showed the checks were not enough: it passed them all with a hair-coloured face and no arms to be seen, which is where the two checks above came from. It still fails `limbs-shown`, and in its run and attack the arms cut through the chest: a part turns about its bone's joint, which is no longer where the cubes' joint was put. As a smooth model it fails the stretch check in every clip; a stout body on the slim skeleton's clips is a problem either way.

Where the eight stand, 2026-11-10: player and wolf pass. Goblin fails `loose` and `speckle` (ragged cloth, busy skin). Deer fails `limbs`. Bear, boar and troll fail `triangles`. Rabbit fails nearly everything, being 8 cubes tall.

### How it is cut

- **A dozen parts, not every bone.** Games that draw characters in cubes build them from a few clean rigid pieces; cutting ours bone by bone (46 of the player's 65) left ragged seams and fused the legs into one mass, at any cube size. So the figure is cut into hips, chest, head and each limb in up to three pieces, read from the skeleton's roles: 15 parts for the player. Fingers, toes, ears and collar bones go with what they hang from.
- **Each part is cut on its own.** The model is stood as in its idle; a part within 25° of an axis is straightened onto it (a leg that leans stands straight, a wolf's sloping neck is left); its surface is sampled into the 5 cm grid and its inside filled. Then each pair of legs is moved apart a cube at a time until there is a column of air between them. A cell two parts both fill goes to the one nearer the root. A cube is fixed to its part's bone and never bends.
- **Each part is tidied**, as someone drawing in cubes would: a cube holding on by one face or none is taken off, twice over; a cube whose colour no neighbour shares takes the colour most of them have.
- **A cube takes the colour most of the texture under it has**, not the average, which is a colour the model has nowhere.
- **Faces are flat and placed.** A head of true size is four cubes wide and an eye is smaller than a cube, so a face copied from the model is a smear. For a creature with arms the front of the head is made one plane of one skin colour (the commonest on the bottom third of the head's front; hair can be the top half), from the chin up to the highest row that is nearly half skin. On it go single cubes: two eyes on the top row either side of the middle, a nose under them one cube proud, a mouth lower. An animal keeps its muzzle and has its eyes darkened where the texture is near black.
- **Not done:** flat faces are not merged (so big bodies fail `triangles`), parts pass through each other at joints and gaps open there, and nothing checks the figure in motion.

## Known limits

- Made gaits are regular: every step is the same, and a made biped walk is stiffer than a recorded one. They are the place to add craft (a head that leads a turn, a gallop's gathered and stretched phases held longer).
- The gallop is one kind (transverse). A rabbit gallops like a small deer; it does not hop.
- `die` leaves legs stiff.
- A biped's `die` and `hurt`, and `attack` for animals, have been judged on sheets, not in play.
- The Tripo CLI drives the web app's private backend with a browser cookie that lasts about 30 days ([tools/tripo/README.md](../tools/tripo/README.md)). When builds fail with a login error, the cookie in `.env` needs replacing.
