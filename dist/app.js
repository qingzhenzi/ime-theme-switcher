/* 输入法候选框样式 · 主题切换 + 体检初始化 */
(function () {
  "use strict";

  const tauri =
    window.__TAURI__ && window.__TAURI__.core ? window.__TAURI__.core : null;
  const invoke = tauri ? tauri.invoke : null;

  const gridEl = document.getElementById("grid");
  const searchEl = document.getElementById("search");
  const countEl = document.getElementById("count");
  const currentNameEl = document.getElementById("current-name");
  const toastEl = document.getElementById("toast");
  const refreshBtn = document.getElementById("refresh");
  const openConfigBtn = document.getElementById("open-config");
  const healthListEl = document.getElementById("health-list");
  const healthBadgeEl = document.getElementById("health-badge");
  const runAllBtn = document.getElementById("run-all");
  const recheckBtn = document.getElementById("recheck");
  const autostartEl = document.getElementById("autostart");
  const shortcutAccelEl = document.getElementById("shortcut-accel");
  const registerShortcutBtn = document.getElementById("register-shortcut");
  const fixLogEl = document.getElementById("fix-log");
  const appMetaEl = document.getElementById("app-meta");

  const tabs = Array.from(document.querySelectorAll(".tab"));
  const views = {
    themes: document.getElementById("view-themes"),
    health: document.getElementById("view-health"),
  };

  let themes = [];
  let current = "";
  let keyword = "";
  let applying = false;
  let toastTimer = null;
  let activeTab = "themes";
  let health = [];
  let busy = false;

  /* ---------- 小工具 ---------- */
  function toCssColor(value, fallback) {
    const v = (value || "").trim();
    const m = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(v);
    if (m) {
      if (m[2]) {
        const r = parseInt(m[1].slice(0, 2), 16);
        const g = parseInt(m[1].slice(2, 4), 16);
        const b = parseInt(m[1].slice(4, 6), 16);
        const a = (parseInt(m[2], 16) / 255).toFixed(3);
        return "rgba(" + r + ", " + g + ", " + b + ", " + a + ")";
      }
      return v;
    }
    if (/^#([0-9a-f]{3})$/i.test(v)) return v;
    return fallback;
  }

  function showToast(message, isError) {
    toastEl.textContent = message;
    toastEl.classList.toggle("error", !!isError);
    toastEl.classList.add("show");
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(function () {
      toastEl.classList.remove("show");
    }, 3200);
  }

  function errorText(err) {
    if (!err) return "未知错误";
    if (typeof err === "string") return err;
    if (err.message) return err.message;
    return String(err);
  }

  function setBusy(value) {
    busy = value;
    runAllBtn.disabled = value;
    recheckBtn.disabled = value;
    registerShortcutBtn.disabled = value;
    refreshBtn.disabled = value;
  }

  /* ---------- 标签页 ---------- */
  function switchTab(tab) {
    activeTab = tab;
    tabs.forEach(function (el) {
      el.classList.toggle("active", el.dataset.tab === tab);
    });
    Object.keys(views).forEach(function (key) {
      views[key].classList.toggle("active", key === tab);
    });
    if (tab === "health") {
      loadHealth();
    }
  }

  tabs.forEach(function (el) {
    el.addEventListener("click", function () {
      switchTab(el.dataset.tab);
    });
  });

  /* ---------- 主题渲染 ---------- */
  function sortThemes(list) {
    return list.slice().sort(function (a, b) {
      if (a.is_current !== b.is_current) return a.is_current ? -1 : 1;
      if (a.source !== b.source) return a.source === "user" ? -1 : 1;
      return String(a.display_name).localeCompare(String(b.display_name), "zh-CN");
    });
  }

  function matches(theme) {
    if (!keyword) return true;
    const hay = [
      theme.name,
      theme.display_name,
      theme.author,
      theme.description,
      theme.source === "user" ? "用户" : "系统",
    ]
      .join(" ")
      .toLowerCase();
    return hay.indexOf(keyword) !== -1;
  }

  function buildSwatch(theme) {
    const wrap = document.createElement("div");
    wrap.className = "swatch";

    const pill = document.createElement("div");
    pill.className = "swatch-pill";
    pill.style.background = toCssColor(
      theme.background_color,
      "rgba(255, 255, 255, 0.06)"
    );
    pill.style.border =
      "1px solid " + toCssColor(theme.border_color, "rgba(255, 255, 255, 0.18)");
    pill.style.color = toCssColor(theme.normal_color, "#d8dee9");

    const hl = document.createElement("span");
    hl.className = "hl";
    hl.textContent = "1 你好";
    hl.style.background = toCssColor(
      theme.highlight_color,
      "rgba(136, 192, 208, 0.35)"
    );
    hl.style.color = toCssColor(theme.highlight_text_color, "#102a32");

    const normal = document.createElement("span");
    normal.textContent = "2 世界 3 输入";

    pill.appendChild(hl);
    pill.appendChild(normal);
    wrap.appendChild(pill);
    return wrap;
  }

  function buildCard(theme) {
    const card = document.createElement("button");
    card.type = "button";
    card.className = "card" + (theme.is_current ? " current" : "");
    card.dataset.name = theme.name;
    card.title =
      "点击切换为「" + theme.display_name + "」" +
      (theme.name !== theme.display_name ? "（" + theme.name + "）" : "");

    const preview = document.createElement("div");
    preview.className = "preview";

    const img = document.createElement("img");
    img.alt = "";
    img.loading = "lazy";
    img.src = "previews/" + encodeURIComponent(theme.name) + ".png";
    img.addEventListener("error", function () {
      preview.classList.add("failed");
    });

    preview.appendChild(img);
    preview.appendChild(buildSwatch(theme));

    const info = document.createElement("div");
    info.className = "info";

    const nameRow = document.createElement("div");
    nameRow.className = "name-row";
    const name = document.createElement("span");
    name.className = "name";
    name.textContent = theme.display_name || theme.name;
    nameRow.appendChild(name);
    if (theme.display_name && theme.display_name !== theme.name) {
      const id = document.createElement("span");
      id.className = "id";
      id.textContent = theme.name;
      nameRow.appendChild(id);
    }

    const subRow = document.createElement("div");
    subRow.className = "sub-row";

    if (theme.is_current) {
      const now = document.createElement("span");
      now.className = "badge now";
      now.textContent = "使用中";
      subRow.appendChild(now);
    }
    const source = document.createElement("span");
    source.className = "badge " + (theme.source === "user" ? "user" : "system");
    source.textContent = theme.source === "user" ? "用户主题" : "系统主题";
    subRow.appendChild(source);

    if (!theme.has_page_buttons) {
      const nopage = document.createElement("span");
      nopage.className = "badge nopage";
      nopage.textContent = "无翻页按钮";
      subRow.appendChild(nopage);
    }

    if (theme.author) {
      const author = document.createElement("span");
      author.className = "author";
      author.textContent = "by " + theme.author;
      subRow.appendChild(author);
    }

    info.appendChild(nameRow);
    info.appendChild(subRow);
    card.appendChild(preview);
    card.appendChild(info);

    card.addEventListener("click", function () {
      applyTheme(theme);
    });
    return card;
  }

  function renderThemes() {
    const visible = sortThemes(themes).filter(matches);
    gridEl.innerHTML = "";

    if (!visible.length) {
      const empty = document.createElement("div");
      empty.className = "empty";
      empty.textContent = keyword ? "没有匹配的主题" : "没有扫描到主题目录";
      gridEl.appendChild(empty);
    } else {
      visible.forEach(function (theme) {
        gridEl.appendChild(buildCard(theme));
      });
    }

    const total = themes.length;
    countEl.textContent = keyword
      ? "匹配 " + visible.length + " / " + total + " 个主题"
      : "共 " + total + " 个主题";

    const now =
      themes.find(function (t) {
        return t.name === current;
      }) || null;
    currentNameEl.textContent = now ? now.display_name || now.name : current || "—";
  }

  /* ---------- 体检渲染 ---------- */
  function levelTag(level) {
    return (
      { ok: "正常", warn: "待处理", error: "错误", info: "说明" }[level] || level
    );
  }

  function renderHealth() {
    healthListEl.innerHTML = "";
    const pending = health.filter(function (item) {
      return item.level === "warn" || item.level === "error";
    }).length;
    healthBadgeEl.textContent = pending > 0 ? String(pending) : "";
    healthBadgeEl.classList.toggle("hidden", pending === 0);

    health.forEach(function (item) {
      const row = document.createElement("div");
      row.className = "check " + item.level;

      const dot = document.createElement("span");
      dot.className = "dot";

      const body = document.createElement("div");
      body.className = "body";
      const title = document.createElement("div");
      title.className = "title";
      title.textContent = item.title;
      const tag = document.createElement("span");
      tag.className = "tag";
      tag.textContent = levelTag(item.level);
      title.appendChild(tag);
      const detail = document.createElement("div");
      detail.className = "detail";
      detail.textContent = item.detail;
      body.appendChild(title);
      body.appendChild(detail);

      row.appendChild(dot);
      row.appendChild(body);

      if (item.fixable) {
        const fixWrap = document.createElement("div");
        fixWrap.className = "fix";
        const fixBtn = document.createElement("button");
        fixBtn.className = "btn";
        fixBtn.textContent = item.fix_label || "修复";
        fixBtn.addEventListener("click", function () {
          runFix(item);
        });
        fixWrap.appendChild(fixBtn);
        row.appendChild(fixWrap);
      }

      healthListEl.appendChild(row);
    });
  }

  function appendFixLog(text) {
    fixLogEl.classList.remove("hidden");
    fixLogEl.textContent = text;
  }

  async function loadHealth() {
    if (!invoke) {
      return;
    }
    try {
      health = await invoke("check_health");
      renderHealth();
    } catch (err) {
      showToast("体检失败：" + errorText(err), true);
    }
  }

  async function runFix(item) {
    if (!invoke || busy) return;
    setBusy(true);
    try {
      const message = await invoke("run_fix", { id: item.id });
      appendFixLog("【" + item.title + "】\n" + message);
      showToast("已处理：" + item.title);
      await Promise.all([loadHealth(), loadThemes()]);
    } catch (err) {
      appendFixLog("【" + item.title + "】\n失败：" + errorText(err));
      showToast("处理失败：" + errorText(err), true);
    } finally {
      setBusy(false);
    }
  }

  async function runAllFixes() {
    if (!invoke || busy) return;
    setBusy(true);
    try {
      const message = await invoke("run_all_fixes");
      appendFixLog("一键初始化结果：\n" + message);
      showToast("一键初始化完成，详情见下方日志");
      await Promise.all([loadHealth(), loadThemes()]);
    } catch (err) {
      appendFixLog("一键初始化失败：" + errorText(err));
      showToast("一键初始化失败：" + errorText(err), true);
    } finally {
      setBusy(false);
    }
  }

  async function loadAutostart() {
    if (!invoke) return;
    try {
      autostartEl.checked = await invoke("get_autostart");
    } catch (err) {
      /* ignore */
    }
  }

  /* ---------- 主题操作 ---------- */
  async function applyTheme(theme) {
    if (applying) return;
    if (theme.is_current) {
      showToast("已经是「" + (theme.display_name || theme.name) + "」了");
      return;
    }
    if (!invoke) {
      showToast("需要在 Tauri 应用里运行才能切换主题", true);
      return;
    }
    applying = true;
    try {
      const message = await invoke("apply_theme", { name: theme.name });
      current = theme.name;
      themes = themes.map(function (t) {
        t.is_current = t.name === theme.name;
        return t;
      });
      renderThemes();
      showToast(String(message || "已切换"));
    } catch (err) {
      showToast("切换失败：" + errorText(err), true);
    } finally {
      applying = false;
    }
  }

  async function loadThemes() {
    if (!invoke) {
      renderThemes();
      return;
    }
    try {
      const [list, now] = await Promise.all([
        invoke("list_themes"),
        invoke("current_theme"),
      ]);
      themes = Array.isArray(list) ? list : [];
      current = typeof now === "string" ? now : "";
      renderThemes();
    } catch (err) {
      showToast("读取主题失败：" + errorText(err), true);
    }
  }

  async function loadAppInfo() {
    if (!invoke) {
      appMetaEl.textContent = "v0.2.0 · 浏览器预览模式";
      return;
    }
    try {
      const info = await invoke("app_info");
      appMetaEl.textContent =
        "v" + (info.version || "?") + " · 仅在 " + (info.tested_on || "CachyOS") + " 测试过";
      if (info.shortcut) {
        shortcutAccelEl.value = info.shortcut;
      }
    } catch (err) {
      /* ignore */
    }
  }

  /* ---------- 事件 ---------- */
  searchEl.addEventListener("input", function () {
    keyword = searchEl.value.trim().toLowerCase();
    renderThemes();
  });

  refreshBtn.addEventListener("click", function () {
    loadThemes().then(function () {
      showToast("已重新扫描主题目录");
    });
  });

  openConfigBtn.addEventListener("click", async function () {
    if (!invoke) return;
    try {
      await invoke("open_fcitx_config");
    } catch (err) {
      showToast("打开失败：" + errorText(err), true);
    }
  });

  recheckBtn.addEventListener("click", function () {
    loadHealth().then(function () {
      showToast("体检完成");
    });
  });

  runAllBtn.addEventListener("click", runAllFixes);

  autostartEl.addEventListener("change", async function () {
    if (!invoke) return;
    try {
      const message = await invoke("set_autostart", {
        enabled: autostartEl.checked,
      });
      showToast(message);
      loadHealth();
    } catch (err) {
      autostartEl.checked = !autostartEl.checked;
      showToast("设置失败：" + errorText(err), true);
    }
  });

  registerShortcutBtn.addEventListener("click", async function () {
    if (!invoke) return;
    const accel = (shortcutAccelEl.value || "").trim();
    try {
      const message = await invoke("register_shortcut", { accel });
      showToast(message);
      loadHealth();
    } catch (err) {
      showToast(errorText(err), true);
      loadHealth();
    }
  });

  /* ---------- 启动 ---------- */
  loadAppInfo();
  loadAutostart();
  loadThemes();
  loadHealth();
})();
