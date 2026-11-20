# The text size set through the settings window (KINIG-160, ADR-0022). Sourced by lib.sh at its end (moved out of it so that lib.sh stays under
# the size cap); it uses `drive_raw`, `driver_query`, `fail`, `sql` and the variables of lib.sh.

# drive_any <window> <step kind> <AX role or ""> <more JSON fields or ""> <name>...: sends the step with the first name the window has (the
# same label in each interface language). Returns 1 when none had it. No scenario error: the caller decides.
drive_any() {
  local window="$1" kind="$2" role="$3" more="$4" name
  shift 4
  for name in "$@"; do
    drive_raw "$window" "{\"kind\":\"$kind\",$(driver_query "$name" "$role")$more}" && return 0
  done
  return 1
}

# set_text_size <large|normal> (KINIG-160, ADR-0022): the text size set the way a person sets it, in the settings window, with the screens
# driver. The scenarios used to edit settings.json for this; the file is gone. The steps are the profile menu, "Settings", the radio
# button, and a check of the database. It works in a scenario whatever its own input is (AX or the driver), because it sends driver steps
# directly; the build must have the driver (screens.yml). No step touches agent access, and none can: the driver has no such step.
# The window may already be open on any page (opening it again only raises it), so the helper presses "Appearance" itself. The driver has
# no step to close a window, so the settings window stays open, but the helper ends by putting the MAIN window in front (the driver's
# `front` step), on every path: an accessibility call without AX_WINDOW acts on the frontmost window, and a resize must not hit the
# settings window. AX_WINDOW is left as it was.
set_text_size() {
  _set_text_size_steps "$@"
  local status=$?
  drive_raw main '{"kind":"front"}' > /dev/null
  sleep 1
  return "$status"
}

_set_text_size_steps() {
  local size="$1" names pages tries stored ready=0
  case "$size" in
    large) names=("Large" "Крупный") ;;
    normal) names=("Normal" "Обычный") ;;
    *) fail "set_text_size: large or normal, not '$size'"; return 1 ;;
  esac
  pages=("Appearance" "Оформление")
  drive_any main press "" "" "Profile" "Профиль" || { fail "set_text_size: no profile button: $DRIVE_ERROR"; return 1; }
  drive_any main press "" "" "Settings" "Настройки" || { fail "set_text_size: no Settings item: $DRIVE_ERROR"; return 1; }
  # The settings window is created by the press above and says "ready" when its page listens: a step before that answers "no such window".
  for tries in $(seq 1 30); do
    if drive_any settings wait "" ',"timeoutMs":2000' "${pages[@]}"; then
      ready=1
      break
    fi
    sleep 1
  done
  [ "$ready" -eq 1 ] || { fail "set_text_size: the settings window has no '${pages[0]}' item: $DRIVE_ERROR"; return 1; }
  drive_any settings press Button "" "${pages[@]}" || { fail "set_text_size: Appearance not opened: $DRIVE_ERROR"; return 1; }
  drive_any settings wait RadioButton ',"timeoutMs":4000' "${names[@]}" || { fail "set_text_size: no '${names[0]}' radio button on Appearance: $DRIVE_ERROR"; return 1; }
  drive_any settings press RadioButton "" "${names[@]}" || { fail "set_text_size: not pressed: $DRIVE_ERROR"; return 1; }
  sleep "$PAUSE"
  stored="$(sql "select value from app_setting where key = 'text_size'")"
  # "Normal" is the default: a database that never held a size answers nothing.
  if [ "$stored" != "$size" ] && ! { [ "$size" = normal ] && [ -z "$stored" ]; }; then
    fail "set_text_size: the database holds '$stored', not '$size'"
    return 1
  fi
  sleep 2
}
