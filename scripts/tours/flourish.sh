# Level 1 flourish tour for scripts/record-run: summon Giant Steps (5 toots), hop right through
# the nugget line (fired up; the even hops may also start a waltz), into the pit a few times
# (wah-wah; the laughing band; the nervous band). ~60-80 s.
hop_right() { for _ in $(seq "$1"); do key ArrowRight Space 300; key ArrowRight 450; done; }
pause 4
for _ in 1 2 3 4 5; do
  key Space 140; pause 0.12; key Space 140; pause 0.9
done
pause 5
hop_right 6
hop_right 6
pause 4
hop_right 10
pause 4
hop_right 10
pause 8
