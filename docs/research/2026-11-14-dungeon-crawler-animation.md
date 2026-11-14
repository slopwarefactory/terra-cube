# What another code-made-creature project does about stiff motion (2026-11-14)

Source: github.com/mitsuhiko/dungeon-crawler at 828080e, `blender/` (Python that builds and
animates creatures in Blender) and `src/render/ik.js`, `blendtree.js`, `stride.js`, `tests/`.
Read, not run: these are its techniques as written, not motion we have seen. The developer
pointed at it: "theres lots of prior art and learnings to be had".

## What they do that bears on our faults

- **Hips.** Height is the inverted pendulum over the stance leg (`min` over stance legs of
  `sqrt(reach² - y²)`, reach 0.975 of the leg) and then low-passed (a Gaussian over the
  cycle). Ours was the same rule taken raw, a drop at every footfall; it is on a spring since
  today (`Biped::carry`). Same finding, same cure.
- **Swing.** `foot_track`: stance moves back at exactly ground speed; swing is a Hermite curve
  whose end tangents equal the stance velocity, so a foot neither stops nor starts at
  lift-off and touch-down; lift is `sin(pi*u^skew)`, skew 0.85 walking and 0.7 running.
- **Run.** A spring-mass, not footfalls: ballistic flight (`v_up = g*t_flight/2`) and a sine
  compression in stance (`v_up*t_stance/pi`).
- **The upper body is what makes it flow.** Spine, head, upper arms and forearms are damped
  spring pendulums hung from the moving pelvis and aimed at the authored pose (walk arms
  k=55, zeta=0.22; run 320, 0.45; spine 700, 0.45; head 380, 0.30). Ours are a recorded
  clip's from the chest up over a walker's legs, with nothing joining the two.
- **Feet on ground at run time** (`plantFeet`, `holdFeet`): each foot's ground offset eased
  (rate 14) and faded out as the clip lifts the foot; the pelvis eased down (rate 10) to the
  lowest foot; a planted foot is locked with hysteresis (down under 0.07 of standing hip
  height, up over 0.13) and takes a step of its own when it is 0.3 of a leg from the clip.
  We eased each foot today and put legs in the ground: we did not fade the offset with the
  clip's lift, which is what lets theirs lag safely.
- **IK.** Soft reach past 90% (`s + k*(1-exp(-(d-s)/k))`), so a leg never snaps straight; the
  pole keeps the side the joint already bends to and never flips.
- **Gaits and speed.** One phase advanced by distance for idle, walk and run; past a clip's
  pace the thighs and upper arms are swung further (slerp past 1), not the clip played faster.
- **Checks that need no eye.** Stance-foot speed against ground speed (skate); loop closure
  per bone; the deformed mesh's lowest point per frame (in the ground); every rig stood half
  on a step with its feet within tolerance; a per-clip film, since "a still frame hides a
  foot skating or a limb snapping".

## What we take

1. Soft IK and a pole that cannot flip, in `cube_anim` (cheap, and a straight-snapping knee
   is part of stiff).
2. Swing matched to stance velocity in `Biped`, and the run as flight and compression.
3. Springs on the spine, head and arms as a layer added at run time over the recorded upper
   body, driven by the pelvis's motion: the clip is not rewritten (the rule stands), it is
   what the springs aim at.
4. The foot's ground offset faded by how high the clip has the foot, then eased, and a
   locked stance foot with hysteresis: the cure for a trunk that jolts at a run.
5. A skate check and a stood-on-a-step check for every creature in `make features`.

Not for us: their meshing and Blender baking, sines tuned by hand per creature.
