#!/bin/sh
# `make cave`: light under the ground, on one sheet (shots/cave.png). The land has no caves of
# its own, so `--cave` digs one into a hillside (`cube_game::cave`): a passage, a hall with a
# shaft to the sky, a pool. Ten fixed views: the mouth, the passage by day and by torch, the
# way back out, the hall with the sun down the hole in its roof (two), the hole itself, the
# hall by torch by day, and at night by a torch and by a fire (`--fire`) from the same place. Look at it after any change to exposure, the sky's fill, the torch or
# what is under a roof (`world::Roofed`).
#   tools/cave.sh [name ...]     only those views (no sheet)
export DISPLAY=${DISPLAY:-:1}
mkdir -p shots/cave
only="$*"
shot() {
    name=$1; shift
    [ -z "$only" ] || echo " $only " | grep -q " $name " || return 0
    out=$(cargo sandbox --scene "shots/cave/$name.png" --play --cave --at 18.5,51 --yaw 3.14 "$@" 2>&1)
    echo "$out" | grep -E '^FAIL|panicked' && return 1
    echo "$out" | grep -o 'pace .*' | sed "s/^pace/cave $name:/"
}
walk='0.5 torch; 3 fwd walk; 23.2 stop'
dark='--hour 23 --day 7'
shot 01-mouth --hour 13 --script '1 stop' --after 8 --eye 20,17,47 --look 20,15.5,57
shot 02-passage --hour 13 --script '1 stop' --after 8 --eye 20,16.4,58 --look 20,14.6,75
shot 03-passage-torch --hour 13 --script '0.5 torch; 3 fwd walk' --after 19 --eye 20.6,15.2,65.5 --look 20,14.0,71
shot 04-way-out --hour 13 --script '1 stop' --after 8 --eye 20,14.3,73.5 --look 20,16.2,56
shot 05-hall-sun --hour 10.4 --script '1 stop' --after 8 --eye 20.4,14.7,74.3 --look 20.4,13.4,82
shot 06-hall-beam --hour 10.4 --script '1 stop' --after 8 --eye 21.8,13.9,77.6 --look 17.8,13.3,81.5
shot 07-hole --hour 13 --script '1 stop' --after 8 --eye 20,13.7,79.3 --look 24,23,87
shot 08-hall-torch --hour 13 --script "$walk" --after 26 --eye 20.4,14.7,74.3 --look 20.4,13.4,82
shot 09-torch-night $dark --script "$walk" --after 26 --eye 20.4,14.7,74.3 --look 20.4,13.4,82
shot 10-fire-night $dark --fire --script '3 fwd walk; 23.2 stop' --after 26 --eye 20.4,14.7,74.3 --look 20.4,13.4,82
[ -n "$only" ] || ffmpeg -hide_banner -loglevel error -y -pattern_type glob -i 'shots/cave/[01][0-9]-*.png' -vf "scale=800:-1,tile=5x2" -frames:v 1 shots/cave.png
