# principal.vy · generates the MAIN sequence map-show.vy plays once (its `done`
# sends the face home). A stand-in for the artist's narrated video.
#   (rendered by ../make-media.sh)

go     = osc /go
p      = ramp go  up 5s  down 1s

field  = circle .3 grid 9 9 soft 1  hue 25 drift 40  alpha (.25 + p*.5)
sun    = circle (.1 + p*.2) soft 1  hue 40  y osc .2hz .05
orbit  = circle .04 soft 1 gray 1  wave .34 .5hz .5hz

all    = field + sun add + orbit add
