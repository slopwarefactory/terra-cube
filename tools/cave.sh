#!/bin/sh
# `make cave`: light under the ground, on one sheet (shots/cave.png). The land has no caves of
# its own, so `--cave` digs one into a hillside (`cube_game::cave`): a passage, a hall with a
# shaft to the sky, a pool. Nine fixed views: the mouth, the passage by day and by torch, the
# hall with the sun down its shaft, by torch from outside and from the eye, by torch at night,
# and the way back out. Look at it after any change to exposure, the sky's fill, the torch or
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
shot 1-mouth --hour 13 --script '1 stop' --after 8 --eye 20,17,47 --look 20,15.5,57
shot 2-passage --hour 13 --script '1 stop' --after 8 --eye 20,16.4,58 --look 20,15.5,75
shot 3-passage-torch --hour 13 --script '0.5 torch; 3 fwd walk' --after 19 --eye 20.6,15.6,65.5 --look 20,14.9,71
shot 4-hall-sun --hour 10.4 --script '1 stop' --after 8 --eye 17,13.2,77 --look 21,13.5,83
shot 5-hall-beam --hour 10.4 --script '1 stop' --after 8 --eye 23.5,14,85 --look 17.5,12.6,80
shot 6-hall-torch --hour 13 --script "$walk" --after 26 --eye 16.5,13.8,76.5 --look 20.5,13.3,81
shot 7-hall-first --hour 13 --script '0.5 torch first; 3 fwd walk; 23.2 stop' --after 26
shot 8-hall-night --hour 23 --day 7 --script "$walk" --after 26 --eye 23.5,14,85 --look 19.5,13,80.5
shot 9-way-out --hour 13 --script '1 stop' --after 8 --eye 20,15.1,73.5 --look 20,16.2,56
[ -n "$only" ] || ffmpeg -hide_banner -loglevel error -y -pattern_type glob -i 'shots/cave/[1-9]-*.png' -vf "scale=800:-1,tile=3x3" -frames:v 1 shots/cave.png
