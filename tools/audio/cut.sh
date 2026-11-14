#!/bin/sh
# How the sounds of the air were made from their recordings, so it can be done again: the
# beds (wind, water) as stretches of long recordings, and the small sounds (each bird's
# calls, frogs, a raven, leaves in a gust) cut out by `cut.py`, which finds them by
# measurement. `cut.sh DIR`, where DIR holds the originals under the names the README of
# assets/audio gives (they are not kept here: tens of megabytes). A source that is not there
# is passed over and said. Then `loops.py --fix` and `level.py --fix`.
here="$(dirname "$0")"; src="${1:?cut.sh DIR: the folder of original recordings}"; out="$here/../../assets/audio"
c() { s="$src/$1"; shift; if [ -s "$s" ]; then python3 -I "$here/cut.py" "$s" "$@"; else echo "passed over: $s"; fi; }
# A bed: SOURCE NAME START SECONDS, in stereo, the rumble under 180 Hz taken off, a second's
# fade at each end (loops.py then joins the end to the start).
bed() { if [ -s "$src/$1" ]; then ffmpeg -hide_banner -loglevel error -y -ss "$3" -t "$4" -i "$src/$1" -af "highpass=f=${5:-180}" -ac 2 -ar 44100 -c:a libvorbis -q:a 4 "$out/$2.ogg" && echo "$2.ogg	$4	$3 s on, of $1"; else echo "passed over: $src/$1"; fi; }

# Wind: a strong wind in trees (three stretches far apart) and wind over a mountain lake (two).
W=yell-040201LowerGeyserBasinWindInTreesBinaural01011.mp3
bed $W amb_wind_2 20 32
bed $W amb_wind_3 128 32
bed $W amb_wind_4 236 32
L=romo-WindAmbientGemLakeROMO52516Final1.mp3
bed $L amb_wind_5 30 32
bed $L amb_wind_6 170 32
# Water: a small stream at noon (three stretches far apart), beside the first river.
R=romo-StreamAmbientROMO52516BlackCanyonTrailFinal1.mp3
bed $R amb_water_2 20 32 120
bed $R amb_water_3 130 32 120
bed $R amb_water_4 240 32 120

# Songbirds, a set to each kind.
c Carolina_Wren.ogg song_wren --count 5 --min 0.8 --max 3.5
c Eastern_Meadowlark.ogg song_lark --count 4 --min 0.6 --max 3.5
c yell-YELLWEME20150425ms.mp3 song_lark --count 3 --min 0.6 --max 3.5 --from 5
c Kentucky_Warbler.ogg song_warbler --count 4 --min 0.6 --max 3.5
c Carolina_Chickadee.ogg song_chickadee --count 5 --min 0.5 --max 3.5
c White_breasted_Nuthatch.ogg song_nuthatch --count 4 --min 0.4 --max 3.5
c Turdus_merula_2.ogg song_merle --count 4 --min 0.6 --max 4
c Yellowstone_sound_library_American_Robin_001.mp3 song_robin --count 4 --min 0.5 --max 4
c romo-HETHROMO672017BierstadtLake.mp3 song_thrush --count 2 --min 0.6 --max 4
c romo-HETHROMO612016AllensParkTrail.mp3 song_thrush --count 2 --min 0.6 --max 4 --from 3
c romo-HETHROMO6102015FernLakeTrail.mp3 song_thrush --count 2 --min 0.6 --max 4 --from 5
c Black_capped_Chickadee_201947598.wav song_tit --count 3 --min 0.3 --max 3 --clear 9
c House_Sparrow.ogg song_sparrow --count 4 --min 0.2 --max 3
# Heard from far: a raven and a crow, woodpeckers, a dove; an owl by night.
c yell-RavenBlackSandBasin.mp3 far_crow --count 4 --min 0.2 --max 3 --high 400
c American_Crow.ogg far_crow --count 3 --min 0.2 --max 3 --high 400 --from 5
c romo-HAWO2ROMO5302017BeaverPonds.mp3 far_drum --count 4 --min 0.4 --max 2.5 --high 300 --low 5000
c Dove_cooing.ogg far_dove --count 3 --min 0.4 --max 4 --high 200 --low 2500
c Barred_Owl_Yellowstone_National_Park.oga owl --count 2 --min 0.5 --max 5 --high 200 --low 3000
c Otus_sunia.ogg owl --count 3 --min 0.2 --max 3 --high 300 --low 4000 --from 3
# Frogs, added to the two the frog itself has.
c yell-20150427T13BorealfrogSACRWISN.mp3 frog --count 4 --min 0.2 --max 2.8 --gap 0.15 --high 500 --from 3
c Edible_frogs_sounds._May._The_Village_of_Krzemienica_d_Voivodeship_Poland.ogg frog --count 3 --min 0.2 --max 2.5 --gap 0.15 --high 300 --from 7
# The insects of a summer's day.
c Grasshoppers.ogg insect --count 3 --swell 1.8 --high 5000
c Cicada_orni.ogg insect --count 3 --swell 2.6 --high 2500 --from 4
c Hummel_bee.ogg insect --count 2 --swell 2.2 --high 120 --low 3000 --from 7
# Leaves in a gust: the loudest stretches of the wind in the trees, without its low end.
c $W rustle --count 5 --swell 3.2 --high 700
# The birds that walk: a mallard's quack, a heron's croak, and a pheasant's crow whole (a
# second long: too short for `cut.py` to find its quiet).
c romo-MALLROMO672017BierstadtLake.mp3 duck --count 2 --min 0.12 --max 1.5 --high 400 --gap 0.15
c Great_Blue_Heron.ogg heron --count 2 --min 0.15 --max 2 --high 300 --gap 0.15
[ -s "$src/Phasianus_colchicus.ogg" ] && ffmpeg -hide_banner -loglevel error -y -i "$src/Phasianus_colchicus.ogg" -af "highpass=f=300,afade=t=in:d=0.02,afade=t=out:st=0.9:d=0.09" -ac 1 -ar 44100 -c:a libvorbis -q:a 4 "$out/pheasant_1.ogg"

# What cutting and falling sound like, from recordings (BigSoundBank, CC0: an axe on a log,
# an axe on wood, twigs broken, a stone let fall), in place of the noise `make.sh` made. A
# blow is heard at once and its ring dies away: `--lead 0.015 --tail 0.3`.
# An axe in a tree: the dull knock of an axe on a log. The blow that breaks something (limbs
# off, a trunk into logs, a log into hunks): the bright crack of an axe on dry wood.
c bigsoundbank-0705.ogg chop --count 6 --min 0.04 --max 1.2 --high 60 --gap 0.15 --lead 0.015 --tail 0.3
c bigsoundbank-0536.ogg split --count 4 --min 0.04 --max 1.5 --high 60 --gap 0.15 --lead 0.015 --tail 0.3
# Stone come down: a stone let fall on stone, with its bounces.
c bigsoundbank-1022.ogg rubble --count 5 --min 0.08 --max 2 --high 60 --gap 0.2 --lead 0.015 --tail 0.2
# From these, slowed (wood twice the size sounds an octave lower and lasts twice as long).
# SOURCE NAME START SECONDS RATE [more filters]: a stretch played RATE times as fast.
slow() { if [ -s "$1" ]; then ffmpeg -hide_banner -loglevel error -y -ss "$3" -t "$4" -i "$1" -af "highpass=f=60,asetrate=44100*$5,aresample=44100${6:+,$6},afade=t=in:d=0.005,areverse,afade=t=in:d=0.25,areverse" -ac 1 -ar 44100 -c:a libvorbis -q:a 4 "$out/$2.ogg" && echo "$2.ogg	of $(basename "$1") from $3 s for $4 s at $5 of its speed"; else echo "passed over: $1"; fi; }
# A tree beginning to go: twigs breaking, at half speed.
slow "$src/bigsoundbank-1300.ogg" crack_1 0 1.2 0.5
slow "$src/bigsoundbank-1299.ogg" crack_2 0.1 1.3 0.5
# Wood come down (a log, a hunk): the knock of the axe on a log at under half speed, its top taken off.
for n in 1 2 3; do slow "$out/chop_$n.ogg" thud_$n 0 0.6 0.45 "lowpass=f=1400"; done
# A tree come down: the same knock at a third of its speed, and over it branches breaking.
if [ -s "$src/bigsoundbank-1299.ogg" ] && [ -s "$out/chop_1.ogg" ]; then
  ffmpeg -hide_banner -loglevel error -y -i "$out/chop_1.ogg" -ss 0.1 -t 1.9 -i "$src/bigsoundbank-1299.ogg" -filter_complex "[0]asetrate=44100*0.3,aresample=44100,lowpass=f=900,volume=2.5[low];[1]highpass=f=120,asetrate=44100*0.7,aresample=44100,adelay=60,volume=0.7[twigs];[low][twigs]amix=inputs=2:normalize=0:duration=longest,afade=t=in:d=0.005,areverse,afade=t=in:d=0.8,areverse" -ac 1 -ar 44100 -c:a libvorbis -q:a 4 "$out/crash_1.ogg" && echo "crash_1.ogg	chop_1 at 0.3 of its speed under bigsoundbank-1299.ogg at 0.7"
fi
# A spade in earth: a shovel dug into the ground. A pick on stone: a shovel struck, iron on
# what is hard (no pick on rock was found).
c bigsoundbank-1305.ogg dig --count 5 --min 0.15 --max 1.6 --high 60 --gap 0.2 --lead 0.02 --tail 0.15
c bigsoundbank-1084.ogg pick --count 4 --min 0.04 --max 1.5 --high 80 --gap 0.15 --lead 0.015 --tail 0.3
# Rain: a summer rain (three stretches far apart) and rain on standing water (one).
bed bigsoundbank-1019.ogg amb_rain_1 8 32 120
bed bigsoundbank-1019.ogg amb_rain_2 58 32 120
bed bigsoundbank-1019.ogg amb_rain_3 112 32 120
bed bigsoundbank-1290.ogg amb_rain_4 20 32 120
