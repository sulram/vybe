# map-show.vy · one face of a projected cube, laptop edition: scenes
# driven by a sensor, a keystone, and the remote that calibrates it.
#
# Same scenes, same transitions, same `out` line as the wall. What differs is
# only what 0.0.2 can't decode yet: the three videos are stood in for by
# generative nodes and by PNG sequences that vybe renders itself.
#
#   examples/patches/map-show/make-media.sh                                  # once
#   vybe run examples/patches/map-show/map-show.vy --key space=/hands  # the face
#   cargo run -p vybe-remote                                                         # the remote
#
# Hold SPACE = two people holding hands. Hold it 3 s and the main piece plays;
# let go early and the face falls back to its screensaver.

hands  = osc /hands            debounce .08
t      = ramp hands            up 3s  down 1.2s

# screensaver — `agua` and `folhas` stand in for agua.mp4 and folhas.mp4
bg     = rect 1 1                       hue 215 gray .16  # the face, edge to edge
agua   = circle .34 grid 10 10 soft 1   hue 210 drift 4   alpha .45
folhas = circle .09 soft 1              hue 130           wave .3 .05hz .08hz   y osc .07hz .03
aura   = circle .06 soft 1              hue 160 drift 6   wave .25 .05hz .08hz
       | feedback decay .3 angle .2 scale .9
trans  = frames media/transicao/*.png
main   = frames media/principal/*.png                     # stands in for principal.mp4

ss     = bg + agua + folhas*.6 add + aura add
ring   = circle .45 stroke .008 gray 1                    # until `rust ring` can draw an arc
touch  = ss*(1-t) + trans@smooth(t) + ring*t

idle   : ss
touch  : touch
play   : main

idle  -> touch   hands rise            cut
touch -> play    t = 1                 cut   confirma.wav
touch -> idle    t = 0 & hands off     cut
play  -> idle    main done             fade 1.2s

# calibration patterns: generated on the GPU, switched by OSC from the remote
white = rect 1 1  gray 1
gray  = rect 1 1  gray .5
grid  = rect 1 1 gray .04 + circle .12 grid 12 12 gray 1
      + line -.5 0 .5 0 stroke .003 gray 1 + line 0 -.5 0 .5 stroke .003 gray 1
      + rect 1 1 stroke .006 gray 1
mode  = osc /mode
*     -> white   mode = white   cut
*     -> gray    mode = gray    cut
*     -> grid    mode = grid    cut
*     -> idle    mode = show    cut

out kms HDMI-A-1 1920x1200 60  picture 1200x1200  keystone config/keystone.json  remote 9001
