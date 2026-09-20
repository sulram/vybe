# scenes.vy · scenes and transitions, driven by a key.
# A scene is a named picture; a transition fires on the frame its condition
# BECOMES true while its `from` scene shows. Hold SPACE to go, let go to return.
#
#   vybe run examples/patches/scenes/scenes.vy
#
# `/key/space` is an input like any other: the window writes every key and the
# mouse into the same address space OSC lands in.

held  = osc /key/space  debounce .05       # a gate: 0 or 1
t     = ramp held  up 2s  down .8s         # climbs while held, falls when released

calm  = circle .25 grid 12 12  gray .4
storm = circle .07 soft 1  hue 330 drift 60  wave .32 .4hz .55hz
      | feedback decay .25 angle 1.4 scale .8
ring  = circle .3 stroke .01  hue 330      # how far along the ramp is

waiting : calm
charged : calm*(1-t) + storm*smooth(t) add + ring*t
burst   : storm

waiting -> charged   held rise           cut
charged -> burst     t = 1               fade .6s
charged -> waiting   t = 0 & held off    cut
burst   -> waiting   held off            fade 1.5s

out window 800x800
