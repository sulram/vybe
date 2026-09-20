# stack.vy · composition: `+` stacks (the right side on top), `*` is opacity,
# `add` sums light instead of covering. And a Scalar plugs into any number:
# `breath` is an oscillator, wired into an opacity and a position.
#
#   vybe run examples/patches/stack/stack.vy

breath = osc .2hz .5                          # a sine, -.5 .. +.5

floor  = circle .22 grid 16 16  gray .35      # a quiet field of dots
sun    = circle .16 soft 1  hue 35  y breath  # rides the oscillator
moon   = circle .10 soft .4 hue 220 x osc .13hz .3
frame  = rect 1 1 stroke .006 gray .8

sky    = sun add + moon*(.5 + breath) add     # two lights, summed
all    = floor + sky + frame
