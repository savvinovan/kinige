// Accessibility instead of mouse and keyboard (macOS, System Events): presses nothing globally, only AX actions on the process window.
//   osascript -l JavaScript scripts/screens/ax.js dump   <pid>
//   osascript -l JavaScript scripts/screens/ax.js press  <pid> <text in the title/description> [role: Button|CheckBox|RadioButton|PopUpButton|MenuItem]
//   osascript -l JavaScript scripts/screens/ax.js scroll <pid> <text>      (AXScrollToVisible)
//   osascript -l JavaScript scripts/screens/ax.js has    <pid> <text>      (prints yes/no)
//   osascript -l JavaScript scripts/screens/ax.js frame  <pid> <text>      (prints "x y width height" of the first match)
//   osascript -l JavaScript scripts/screens/ax.js resize <pid> <width> <height>  (resizes the window; prints the resulting size)
//   osascript -l JavaScript scripts/screens/ax.js set    <pid> <text> <value>   (sets AXValue of the first text field, search field or combo box that matches)
//   osascript -l JavaScript scripts/screens/ax.js menu   <pid> <menu names> <item names>  (AXPress on a menu bar item; names are alternatives separated by "|",
//                                                  the app menu is localized: "Edit|Правка" "Paste|Вставить")
// Process window: the first one by default; the AX_WINDOW=<part of the title> variable selects the "Settings" or "Kinige" window.
// The text is searched as a substring of the element's "role, title, description, value", the first match in traversal order.
ObjC.import("Foundation");

function run(argv) {
  const [mode, pid, want = "", roleWant = ""] = argv;
  const windowWant = ObjC.unwrap($.NSProcessInfo.processInfo.environment.objectForKey("AX_WINDOW")) || "";
  const se = Application("System Events");
  const proc = se.processes.whose({ unixId: Number(pid) })[0];
  // The role filter belongs to `press`; for `set` and `menu` the same argument is a value or an item name.
  const pressable =
    roleWant && mode === "press"
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

  // A property column for all children of an element in one System Events request (a traversal with one property per request took more
  // than a minute); if that fails, one child at a time.
  function column(el, key, count) {
    try {
      const values = el.uiElements[key]();
      if (values.length === count) return values;
    } catch (e) {}
    const values = [];
    for (let i = 0; i < count; i++) {
      try {
        values.push(el.uiElements[i][key]());
      } catch (e) {
        values.push("");
      }
    }
    return values;
  }

  // A property value as a string. For a group of radio buttons and tabs the value is a reference to the selected element, and String() on it
  // fails with "Can't convert types (-1700)" already outside the column's try; such a traversal broke the whole dump of the Settings window.
  function text(v) {
    if (v === null || v === undefined) return "";
    try {
      return String(v);
    } catch (e) {
      return "";
    }
  }

  // A tree dump (the same line format as before: indent by depth, role, title | description | value | name).
  function dumpTree(el, depth) {
    if (depth > 30) return;
    let roles = [];
    try {
      roles = el.uiElements.role();
    } catch (e) {
      return;
    }
    if (!roles || roles.length === 0) return;
    const columns = ["title", "description", "value", "name"].map((key) => column(el, key, roles.length));
    for (let i = 0; i < roles.length; i++) {
      const label = columns
        .map((values) => text(values[i]))
        .filter(Boolean)
        .join(" | ");
      out.push(" ".repeat(depth) + text(roles[i]) + " " + label.slice(0, 400));
      dumpTree(el.uiElements[i], depth + 1);
    }
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
      if (label.includes(want)) {
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
        if (mode === "set" && /^AX(ComboBox|TextField|TextArea|SearchField)$/.test(p.role)) {
          k.value = roleWant;
          return "set: " + p.role + " " + label.slice(0, 60);
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
  if (mode === "menu") {
    // The menu bar, not the window: an AX press on a menu item is not a key.
    for (const menuName of want.split("|")) {
      for (const itemName of roleWant.split("|")) {
        try {
          proc.menuBars[0].menuBarItems.byName(menuName).menus[0].menuItems.byName(itemName).click();
          return "clicked: " + menuName + " > " + itemName;
        } catch (e) {}
      }
    }
    return "no menu item: " + want + " > " + roleWant;
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
  if (mode === "dump") {
    dumpTree(win, 0);
    return out.join("\n");
  }
  const found = walk(win, 0);
  if (mode === "has") return found || "no";
  return found || "not found: " + want;
}
