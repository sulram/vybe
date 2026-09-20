# transicao.vy · generates the TRANSITION sequence map-show.vy scrubs with `@`.
# It is scrubbed, not played, so every frame must work as a still: one ramp `p`
# drives everything, 0 -> 1 over the 4 s that get rendered. Alpha is kept.
#   (rendered by ../make-media.sh)

go    = osc /go
p     = ramp go  up 4s  down 1s

halo  = circle (.06 + p*.42) soft 1  hue 160  alpha p
bloom = circle (.02 + p*.30) stroke (.004 + p*.02)  hue 40
core  = circle (p*.12) soft .5 gray 1

all   = halo + bloom add + core add
