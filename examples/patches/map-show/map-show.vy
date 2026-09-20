# map-show.vy · the whole piece, laptop edition: scenes driven by a sensor, a
# video with its sound, a keystone, and the remote that calibrates it.
#
#   examples/patches/map-show/make-media.sh                       # once
#   scripts/face-and-remote.sh examples/patches/map-show/map-show.vy --key space=/hands
#
# Hold SPACE = two people holding hands. Hold it 3 s and the main piece plays,
# picture and sound; let go early and the face falls back to its screensaver.
# When the video ends (`main done`) the face fades home — and the sound fades
# with it, because a video is as loud as its scene is visible.
#
# THE VIDEO IS NOT IN THE REPO. Put any video at media/video/main.mov (or point `main`
# elsewhere) — that folder is git-ignored; `vybe check` says if it can't find it.

hands  = osc /hands            debounce .08
t      = ramp hands            up 3s  down 1.2s

# screensaver — generative, no media needed
bg     = rect 1.7778 1                  hue 215 gray .16  # the picture, edge to edge
agua   = circle .34 grid 10 10 soft 1   hue 210 drift 4   alpha .45
folhas = circle .09 soft 1              hue 130           wave .3 .05hz .08hz   y osc .07hz .03
aura   = circle .06 soft 1              hue 160 drift 6   wave .25 .05hz .08hz
       | feedback decay .3 angle .2 scale .9
trans  = frames media/transicao/*.png                     # scrubbed by the ramp
main   = video media/video/main.mov  fit  vol .8

ss     = bg + agua + folhas*.6 add + aura add
ring   = circle .45 stroke .008 gray 1                    # until `rust ring` can draw an arc
touch  = ss*(1-t) + trans@smooth(t) + ring*t

idle   : ss
touch  : touch
play   : main

idle  -> touch   hands rise            cut
touch -> play    t = 1                 cut
touch -> idle    t = 0 & hands off     cut
play  -> idle    main done             fade 1.2s

# calibration patterns: generated on the GPU, switched by OSC from the remote
white = rect 1.7778 1  gray 1
gray  = rect 1.7778 1  gray .5
grid  = rect 1.7778 1  hue 215 gray .10
      + line -.6667 -.5 -.6667 .5  stroke .002 gray .7 + line .6667 -.5 .6667 .5  stroke .002 gray .7
      + line -.4444 -.5 -.4444 .5  stroke .002 gray .7 + line .4444 -.5 .4444 .5  stroke .002 gray .7
      + line -.2222 -.5 -.2222 .5  stroke .002 gray .7 + line .2222 -.5 .2222 .5  stroke .002 gray .7
      + line -.8889 -.2222 .8889 -.2222  stroke .002 gray .7 + line -.8889 .2222 .8889 .2222  stroke .002 gray .7
      + line -.8889 0 .8889 0      stroke .003 gray 1
      + line 0 -.5 0 .5            stroke .003 gray 1
      + circle .25 stroke .003 gray 1
      + rect 1.7778 1  stroke .008 hue 40
mode  = osc /mode
*     -> white   mode = white   cut
*     -> gray    mode = gray    cut
*     -> grid    mode = grid    cut
*     -> idle    mode = show    cut

out kms HDMI-A-1 1920x1200 60  picture 1920x1080  keystone config/keystone.json  remote 9001
