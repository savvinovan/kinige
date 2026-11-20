# The GIF steps of the slice 5c scenario (KINIG-151), sourced by 5c.sh: Reduce Motion on the runner, the click path and the autoplay of the small GIF in
# the real window, and the memory of the web content process with the heavy GIFs playing. It uses the helpers and variables of 5c.sh and lib.sh.
#
# The label of a GIF that fits the autoplay limits (cat-small.gif: 320 x 240, 8 frames, far under 8 MiB and 2 Mpx) asks for a click ("Press to play the GIF")
# only when the page reports Reduce Motion (`GifCell`: `waitsForClick`), so the label is the window's own report. Under Reduce Motion this proves the click
# path: still, pressed, moving, scrolled away, still again. Without it, the autoplay path: moving on its own. "Moving" is two shots half a second apart that
# differ (`image-diff.py`), in a box as big as a picture and where the picture is (a caret blinking in a field differs too); the control is the same pair
# with nothing playing.

SMALL="cat-small.gif, 320 на 240"
SMALL_WAITS="$SMALL. Нажмите, чтобы воспроизвести GIF"
rm_state() { if has "$SMALL_WAITS"; then echo reduced; else echo "not reduced"; fi; }
RM="$OUT/reduce-motion.txt"
RM_ORIG="$(defaults read com.apple.universalaccess reduceMotion 2> /dev/null)"
RM_CHANGED=0
# Puts the setting back as it was (the trap of 5c.sh calls it when the scenario ended after a change).
restore_motion() {
  if [ -n "$RM_ORIG" ]; then
    defaults write com.apple.universalaccess reduceMotion -bool "$([ "$RM_ORIG" = "1" ] && echo true || echo false)"
  else
    defaults delete com.apple.universalaccess reduceMotion
  fi > /dev/null 2>&1
}

scroll_to "cat-small.gif"
sleep 3
STATE_START="$(rm_state)"
{
  echo "# Reduce Motion on the runner and what the small GIF (cat-small.gif) does in the real window"
  echo "the window's report at the start (the label of a GIF of that size asks for a click only when the page sees prefers-reduced-motion): $STATE_START"
  echo "defaults read com.apple.universalaccess reduceMotion: ${RM_ORIG:-not set}"
} > "$RM"
CELL=""
if [ "$STATE_START" = "reduced" ]; then
  moves_between click-control # the control: nothing plays, so the two shots are the same
  echo "Reduce Motion, nothing pressed, two shots 0.5 s apart: $MOVES" >> "$RM"
  [ "$MOVES_CODE" = "0" ] || fail "the window moves while no GIF plays, so a moving GIF would prove nothing: $MOVES"
  press "$SMALL" button
  sleep 2
  if has "$SMALL" && ! has "$SMALL_WAITS"; then echo "pressed: the label no longer asks for a click" >> "$RM"; else fail "pressing cat-small.gif did not start it (its label still asks for a click)"; fi
  shoot 10b-gif-pressed
  MOVES_OPTS="--min-box=100x100"
  moves_in_three click-playing
  echo "pressed, two shots 0.5 s apart: $MOVES" >> "$RM"
  [ "$MOVES_CODE" = "1" ] || fail "the pressed GIF does not move in the real window: $MOVES"
  CELL="$(printf '%s' "$MOVES" | sed -n 's/.*box \([0-9]*,[0-9]*,[0-9]*,[0-9]*\).*/\1/p')" # where the picture is
  MOVES_OPTS=""
  scroll_to "17 ноября" # far away and back: the click is forgotten when the GIF leaves the screen, so it asks again
  sleep 3
  scroll_to "cat-small.gif"
  sleep 3
  if has "$SMALL_WAITS"; then echo "scrolled away and back: the label asks for a click again" >> "$RM"; else fail "after scrolling away the GIF still plays: it should ask for a click again"; fi
else
  echo "Reduce Motion is off here: the autoplay path is tried below without changing anything" >> "$RM"
fi

# Autoplay, the second half: where Reduce Motion is on, turn it off. Only on a GitHub runner, so the scenario can never change a person's Mac, and the
# setting is put back when the script ends (`restore_motion`). Whether the runner allows it is the window's report after a restart of the app.
if [ "$STATE_START" = "reduced" ]; then
  if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    RM_CHANGED=1
    defaults write com.apple.universalaccess reduceMotion -bool false > "$K/debug/reduce-motion-write.txt" 2>&1
    echo "defaults write com.apple.universalaccess reduceMotion -bool false: exit $?" >> "$RM"
    [ -s "$K/debug/reduce-motion-write.txt" ] && echo "its output: $(head -c 200 "$K/debug/reduce-motion-write.txt")" >> "$RM"
    restart_app
    scroll_to "cat-small.gif"
    sleep 3
  else
    echo "not switched off: this is not a GitHub Actions run (GITHUB_ACTIONS is not true), so the setting of this machine is left alone" >> "$RM"
  fi
fi
STATE_NOW="$(rm_state)"
echo "the window's report now: $STATE_NOW" >> "$RM"
if [ "$STATE_NOW" = "not reduced" ]; then
  # Autoplay: no click is asked for, and it moves on its own while it is on screen. The field's caret is not a candidate: focus goes off the field first,
  # and the box of the change must lie where the GIF was in the click path (or, with no click path, in the feed's column) and be as big as a picture.
  has "$SMALL" && ! has "$SMALL_WAITS" || fail "the small GIF is not found without its click label"
  focus_on "$L_INBOX" button
  shoot 10c-gif-autoplay
  MOVES_OPTS="--min-box=100x100 --within=268,40,1020,605"
  if [ -n "$CELL" ]; then
    IFS=, read -r BX0 BY0 BX1 BY1 <<< "$CELL"
    MOVES_OPTS="--min-box=100x100 --within=$((BX0 - 4)),$((BY0 - 4)),$((BX1 + 4)),$((BY1 + 4))"
  fi
  moves_in_three autoplay
  echo "autoplay, two shots 0.5 s apart (options $MOVES_OPTS): $MOVES" >> "$RM"
  MOVES_OPTS=""
  [ "$MOVES_CODE" = "1" ] || fail "the GIF does not autoplay in the real window: $MOVES"
else
  echo "AUTOPLAY IS UNPROVEN IN THE REAL WINDOW: Reduce Motion stays on for the page here, so the autoplay branch of GifCell did not run (it is proven by jsdom tests only)" >> "$RM"
fi
cat "$RM"

# ---- gif-memory.txt: the web content process while the seven heavy GIFs scroll through, still and then playing ----
web_rss() { ps -axo rss=,comm= | awk '/WebContent/ { if ($1 > m) m = $1 } END { print m + 0 }'; }
HEAVY="wide.gif heavy-5.gif heavy-4.gif heavy-3.gif heavy-2.gif heavy-1.gif heavy-0.gif"
# measure_pass <title> <press: yes|no>: each GIF is scrolled to, pressed when the window waits for a click, and sampled with its movement checked.
measure_pass() {
  local name rss peak=0
  { echo; echo "$1"; echo "before: $(web_rss)"; } >> "$OUT/gif-memory.txt"
  MOVES_OPTS="--min-box=100x100"
  for name in $HEAVY; do
    scroll_quiet "$name"
    sleep 3
    if [ "$2" = "yes" ]; then
      drive_raw "$(drive_window)" "{\"kind\":\"press\",$(driver_query "$name" button)}" > /dev/null 2>&1 || true
      sleep 3
    fi
    moves_between "memory-$name"
    rss="$(web_rss)"
    echo "$name: $rss KB; two shots 0.5 s apart: $MOVES" >> "$OUT/gif-memory.txt"
    [ "$rss" -gt "$peak" ] && peak="$rss"
  done
  MOVES_OPTS=""
  echo "peak: $peak KB" >> "$OUT/gif-memory.txt"
}
{
  echo "# Resident size (KB) of the web content process while the chat 'GIF' scrolls past: six GIFs of about 7.4 MiB and one of 1.96 megapixels, all under the autoplay limits"
  echo "(8 MiB and 2,000,000 pixels). A GIF that 'moves' is one whose two shots 0.5 s apart differ in a box of at least 100 x 100; 'still' is one whose shots are the same."
} > "$OUT/gif-memory.txt"
press "GIF" button
sleep 4
if [ "$STATE_NOW" = "reduced" ]; then
  measure_pass "Pass 1, Reduce Motion on, nothing pressed: first frames only" no
  scroll_quiet "wide.gif"
  measure_pass "Pass 2, Reduce Motion on, each GIF pressed so that it plays" yes
else
  measure_pass "Autoplay (Reduce Motion off for the page): each GIF plays while it is on screen" no
fi
press "$L_INBOX" button
sleep 3
