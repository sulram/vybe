# map-screen.vy · map a 16:9 SCREEN with a 16:10 projector.
#
#   scripts/face-and-remote.sh examples/patches/map-screen/map-screen.vy
#
# Same tool, another shape: `picture 1920x1080` makes the picture 16:9, and the
# keystone's corners are the screen's corners. The remote draws it 16:9 inside
# a 16:10 frame because the face told it so — see map-cube.vy for a square.
#
# Scene space still measures by the SHORTER edge: the picture is 1 tall and
# 16/9 = 1.7778 wide, so x runs -.8889 .. +.8889.
#
# Remote keys: drag a corner | TAB + arrows (SHIFT = 10 px)
#              G grid  W white  H gray  SPACE show | S save  R reload  0 reset

mode  = osc /mode

bg    = rect 1.7778 1  hue 205 gray .22                  # the screen, edge to edge
comet = circle .10 soft 1  hue 30 drift 20  wave .4 .11hz .17hz  x osc .05hz .4
      | feedback decay .3 angle .3 scale .9
show  = bg + comet add

white = rect 1.7778 1  gray 1
gray  = rect 1.7778 1  gray .5

# Squares of .2222 (an eighth of the width): if they look square on the wall,
# the mapping is right.
grid  = rect 1.7778 1  hue 205 gray .12
      + line -.6667 -.5 -.6667 .5  stroke .002 gray .7 + line .6667 -.5 .6667 .5  stroke .002 gray .7
      + line -.4444 -.5 -.4444 .5  stroke .002 gray .7 + line .4444 -.5 .4444 .5  stroke .002 gray .7
      + line -.2222 -.5 -.2222 .5  stroke .002 gray .7 + line .2222 -.5 .2222 .5  stroke .002 gray .7
      + line -.8889 -.4444 .8889 -.4444  stroke .002 gray .7 + line -.8889 .4444 .8889 .4444  stroke .002 gray .7
      + line -.8889 -.2222 .8889 -.2222  stroke .002 gray .7 + line -.8889 .2222 .8889 .2222  stroke .002 gray .7
      + line -.8889 0 .8889 0      stroke .003 gray 1
      + line 0 -.5 0 .5            stroke .003 gray 1
      + line -.8889 -.5 .8889 .5   stroke .002 gray .6
      + line -.8889 .5 .8889 -.5   stroke .002 gray .6
      + circle .25 stroke .003 gray 1
      + rect 1.7778 1  stroke .008 hue 40                # the edge to land on the screen's

idle  : show

*  -> white   mode = white   cut
*  -> gray    mode = gray    cut
*  -> grid    mode = grid    cut
*  -> idle    mode = show    fade .5s

out window 1920x1200  picture 1920x1080  keystone config/keystone.json  remote 9001
