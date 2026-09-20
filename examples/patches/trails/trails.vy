# trails.vy · wire a node into a GPU effect with `|`.
# The circle is the energy; `feedback` is the memory. All three knobs are per
# second: how much of the trail survives, how far it turns, how much it zooms.
#
#   vybe run examples/patches/trails/trails.vy

comet = circle .05 soft 1  hue 190 drift 25  wave .3 .16hz .21hz
      | feedback decay .16 angle .9 scale .74
