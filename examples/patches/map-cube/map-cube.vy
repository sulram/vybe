# map-cube.vy · map one SQUARE face of a cube with a 16:10 projector.
#
#   scripts/face-and-remote.sh examples/patches/map-cube/map-cube.vy
#
# `picture 1200x1200` is the point: the face renders as a square of its own,
# and the keystone's four corners are the corners OF THAT SQUARE — drag them in
# the remote onto the cube's real corners. The projector's other pixels stay
# black. The remote asks the face for its shape, so it draws this one as a
# square inside a 16:10 frame without being told.
#
# Remote keys: drag a corner | TAB + arrows (SHIFT = 10 px)
#              G grid  W white  H gray  SPACE show | S save  R reload  0 reset
#
# Nothing here is a calibration feature: the patterns are ordinary scenes
# switched by an ordinary input (`/mode`). The coloured background is what lets
# you see where the picture ends and the projector's black begins.

mode  = osc /mode

bg    = rect 1 1  hue 275 gray .22                       # the face, edge to edge
comet = circle .10 soft 1  hue 160 drift 20  wave .3 .11hz .17hz
      | feedback decay .3 angle .3 scale .9
show  = bg + comet add

white = rect 1 1  gray 1
gray  = rect 1 1  gray .5
grid  = rect 1 1  hue 275 gray .12
      + circle .12 grid 12 12  gray 1
      + line -.5 0 .5 0     stroke .003 gray 1
      + line 0 -.5 0 .5     stroke .003 gray 1
      + line -.5 -.5 .5 .5  stroke .002 gray .6
      + line -.5 .5 .5 -.5  stroke .002 gray .6
      + circle .25 stroke .003 gray 1
      + rect 1 1  stroke .008 hue 40                     # the edge to land on the cube's

idle  : show

*  -> white   mode = white   cut
*  -> gray    mode = gray    cut
*  -> grid    mode = grid    cut
*  -> idle    mode = show    fade .5s

out window 1920x1200  picture 1200x1200  keystone config/keystone.json  remote 9001
