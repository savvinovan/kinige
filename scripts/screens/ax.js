// Доступность вместо мыши и клавиатуры (macOS, System Events): ничего не жмёт глобально, только действия AX над окном процесса.
//   osascript -l JavaScript scripts/screens/ax.js dump   <pid>
//   osascript -l JavaScript scripts/screens/ax.js press  <pid> <текст в названии/описании> [роль: Button|CheckBox|RadioButton|PopUpButton|MenuItem]
//   osascript -l JavaScript scripts/screens/ax.js scroll <pid> <текст>      (AXScrollToVisible)
//   osascript -l JavaScript scripts/screens/ax.js has    <pid> <текст>      (печатает yes/no)
// Текст ищется как подстрока в «роль, название, описание, значение» элемента, первый найденный в порядке обхода.
function run(argv) {
  const [mode, pid, want = "", roleWant = ""] = argv;
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
  const found = walk(proc.windows[0], 0);
  if (mode === "dump") return out.join("\n");
  if (mode === "has") return found || "no";
  return found || "not found: " + want;
}
