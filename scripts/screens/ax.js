// Доступность вместо мыши и клавиатуры (macOS, System Events): ничего не жмёт глобально, только действия AX над окном процесса.
//   osascript -l JavaScript scripts/screens/ax.js dump   <pid>
//   osascript -l JavaScript scripts/screens/ax.js press  <pid> <текст в названии/описании> [роль: Button|CheckBox|RadioButton|PopUpButton|MenuItem]
//   osascript -l JavaScript scripts/screens/ax.js scroll <pid> <текст>      (AXScrollToVisible)
//   osascript -l JavaScript scripts/screens/ax.js has    <pid> <текст>      (печатает yes/no)
//   osascript -l JavaScript scripts/screens/ax.js frame  <pid> <текст>      (печатает «x y ширина высота» первого найденного)
//   osascript -l JavaScript scripts/screens/ax.js resize <pid> <ширина> <высота>  (меняет размер окна; печатает итоговый размер)
// Окно процесса: по умолчанию первое; переменная AX_WINDOW=<часть заголовка> выбирает окно «Settings» или «Kinige».
// Текст ищется как подстрока в «роль, название, описание, значение» элемента, первый найденный в порядке обхода.
ObjC.import("Foundation");

function run(argv) {
  const [mode, pid, want = "", roleWant = ""] = argv;
  const windowWant = ObjC.unwrap($.NSProcessInfo.processInfo.environment.objectForKey("AX_WINDOW")) || "";
  const se = Application("System Events");
  const proc = se.processes.whose({ unixId: Number(pid) })[0];
  const pressable = roleWant
    ? new RegExp("^AX(" + roleWant + ")$")
    : /^AX(Button|MenuItem|CheckBox|RadioButton|PopUpButton|MenuButton)$/;
  const out = [];

  function parts(el) {
    const p = {};
    for (const key of ["role", "title", "description", "value", "name"]) {
      try {
        const v = el[key]();
        p[key] = v === null || v === undefined ? "" : String(v);
      } catch (e) {
        p[key] = "";
      }
    }
    return p;
  }

  function walk(el, depth) {
    if (depth > 30) return null;
    let kids = [];
    try {
      kids = el.uiElements();
    } catch (e) {
      return null;
    }
    for (const k of kids) {
      const p = parts(k);
      const label = [p.title, p.description, p.value, p.name].filter(Boolean).join(" | ");
      if (mode === "dump") {
        out.push(" ".repeat(depth) + p.role + " " + label.slice(0, 400));
      } else if (label.includes(want)) {
        if (mode === "has") return "yes";
        if (mode === "frame") {
          try {
            const at = k.position();
            const size = k.size();
            return at[0] + " " + at[1] + " " + size[0] + " " + size[1];
          } catch (e) {}
        }
        if (mode === "scroll") {
          try {
            k.actions.byName("AXScrollToVisible").perform();
            return "scrolled: " + label.slice(0, 80);
          } catch (e) {}
        }
        if (mode === "press" && pressable.test(p.role)) {
          k.actions.byName("AXPress").perform();
          return "pressed: " + p.role + " " + label.slice(0, 80);
        }
      }
      const r = walk(k, depth + 1);
      if (r) return r;
    }
    return null;
  }

  if (!proc.exists()) return "no process";
  if (mode === "front") {
    proc.frontmost = true;
    return "frontmost";
  }
  let win = proc.windows[0];
  if (windowWant) {
    win = null;
    const count = proc.windows().length;
    for (let i = 0; i < count; i++) {
      let title = "";
      try {
        title = String(proc.windows[i].name());
      } catch (e) {}
      if (title.includes(windowWant)) {
        win = proc.windows[i];
        break;
      }
    }
    if (!win) return "no window: " + windowWant;
  }
  if (mode === "resize") {
    win.size = [Number(want), Number(roleWant)];
    const size = win.size();
    return size[0] + " " + size[1];
  }
  const found = walk(win, 0);
  if (mode === "dump") return out.join("\n");
  if (mode === "has") return found || "no";
  return found || "not found: " + want;
}
