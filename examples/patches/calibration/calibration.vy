# calibration.vy · a face to aim a projector with — and the patch the keystone
# remote talks to. Window 1 of 2:
#
#   vybe run examples/patches/calibration/calibration.vy      # the face
#   cargo run -p vybe-remote                                  # the remote
#
# In the remote: drag a corner (or TAB + arrows, SHIFT = 10 px); G grid,
# W white, H gray, SPACE back to the show; S saves config/keystone.json next to
# this file, R re-reads it.
#
# Nothing here is a calibration feature: the patterns are ordinary scenes
# switched by an ordinary input (`/mode`), generated on the GPU. `remote 9001`
# is the whole network setup; the corners live in the stage, outside the patch.

mode  = osc /mode

show  = circle .12 soft 1  hue 160 drift 20  wave .3 .11hz .17hz
      | feedback decay .3 angle .3 scale .9

white = rect 1 1  gray 1
gray  = rect 1 1  gray .5
grid  = rect 1 1  gray .04
      + circle .12 grid 12 12  gray 1
      + line -.5 0 .5 0   stroke .003 gray 1
      + line 0 -.5 0 .5   stroke .003 gray 1
      + line -.5 -.5 .5 .5  stroke .002 gray .5
      + line -.5 .5 .5 -.5  stroke .002 gray .5
      + circle .25 stroke .003 gray 1
      + rect 1 1  stroke .006 hue 40

idle  : show

*  -> white   mode = white   cut
*  -> gray    mode = gray    cut
*  -> grid    mode = grid    cut
*  -> idle    mode = show    fade .5s

out window 1920x1200  keystone config/keystone.json  remote 9001
