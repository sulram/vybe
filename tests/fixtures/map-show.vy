# map-show.vy · one cube face — the patch from the 0.0.2 design brief, as
# designed for the wall (video, a rust leaf, KMS). A parser fixture: it must always
# parse. One change from the brief: its node `frames = frames …` is `trans`
# here, because a node can't take a word of the language as its name.

hands  = osc /hands            debounce .08
t      = ramp hands            up 3s  down 1.2s

agua   = video agua.mp4        loop mute
folhas = video folhas.mp4      loop mute        y osc .07hz .03
aura   = circle .06 soft 1     hue 160 drift 6  wave .25 .05hz .08hz
       | feedback decay .3 angle .2 scale .9
trans  = frames transicao/*.png
play   = video principal.mp4   vol .8

ss     = agua + folhas*.6 add + aura add
ring   = rust ring                                # fn ring(t: f32, g: &mut Draw)
touch  = ss*(1-t) + trans@smooth(t) + ring

idle   : ss
touch  : touch
play   : play

idle  -> touch   hands rise            cut
touch -> play    t = 1                 cut   confirma.wav
touch -> idle    t = 0 & hands off     cut
play  -> idle    play done             fade 1.2s

# calibration patterns: generated on the GPU, switched by OSC from the remote
white = rect 1 1  gray 1
gray  = rect 1 1  gray .5
grid  = rect 1 1 gray 0 + circle .01 grid 12 12 gray 1
      + line -.5 0 .5 0 stroke .002 gray 1 + line 0 -.5 0 .5 stroke .002 gray 1
      + rect 1 1 stroke .004 gray 1 + text "NORTE" size .08 gray .6
mode  = osc /mode
*     -> white   mode = white   cut
*     -> gray    mode = gray    cut
*     -> grid    mode = grid    cut
*     -> idle    mode = show    cut

out kms HDMI-A-1 1920x1200 60  keystone config/keystone.json  remote 9001
