<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import ThemeCard from "./components/ThemeCard.vue";
import {
  applyTheme,
  appInfo,
  checkHealth,
  currentTheme,
  errorText,
  getAutostart,
  hasTauri,
  listThemes,
  openFcitxConfig,
  registerShortcut,
  runAllFixes,
  runFix,
  setAutostart,
} from "./api";
import type { AppInfo, CheckItem, ThemeInfo } from "./api";

type TabId = "themes" | "health";

const tabs: { id: TabId; label: string }[] = [
  { id: "themes", label: "主题" },
  { id: "health", label: "体检 / 初始化" },
];

const activeTab = ref<TabId>("themes");
const themes = ref<ThemeInfo[]>([]);
const current = ref("");
const keyword = ref("");
const health = ref<CheckItem[]>([]);
const info = ref<AppInfo | null>(null);
const autostart = ref(false);
const shortcutAccel = ref("Ctrl+Alt+I");
const busy = ref(false);
const fixLog = ref("");
const showFixLog = ref(false);
const toastText = ref("");
const toastError = ref(false);
const toastVisible = ref(false);

let toastTimer: ReturnType<typeof setTimeout> | undefined;
let applying = false;

/* ---------- 小工具 ---------- */
function showToast(message: string, isError = false) {
  toastText.value = message;
  toastError.value = isError;
  toastVisible.value = true;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toastVisible.value = false;
  }, 3400);
}

function setBusy(value: boolean) {
  busy.value = value;
}

function appendFixLog(text: string) {
  fixLog.value = text;
  showFixLog.value = true;
}

/* ---------- 主题 ---------- */
const currentDisplay = computed(() => {
  const hit = themes.value.find((t) => t.name === current.value);
  return hit ? hit.display_name || hit.name : current.value || "—";
});

const visibleThemes = computed(() => {
  const kw = keyword.value.trim().toLowerCase();
  const list = themes.value.slice().sort((a, b) => {
    if (a.is_current !== b.is_current) return a.is_current ? -1 : 1;
    if (a.source !== b.source) return a.source === "user" ? -1 : 1;
    return String(a.display_name).localeCompare(String(b.display_name), "zh-CN");
  });
  if (!kw) return list;
  return list.filter((theme) =>
    [theme.name, theme.display_name, theme.author, theme.description]
      .join(" ")
      .toLowerCase()
      .includes(kw)
  );
});

const countText = computed(() => {
  const total = themes.value.length;
  if (keyword.value.trim()) return `匹配 ${visibleThemes.value.length} / ${total} 个主题`;
  return `共 ${total} 个主题`;
});

async function loadThemes() {
  try {
    const [list, now] = await Promise.all([listThemes(), currentTheme()]);
    themes.value = list;
    current.value = now;
  } catch (err) {
    showToast("读取主题失败：" + errorText(err), true);
  }
}

async function onSelectTheme(theme: ThemeInfo) {
  if (applying) return;
  if (theme.is_current) {
    showToast(`已经是「${theme.display_name || theme.name}」了`);
    return;
  }
  if (!hasTauri()) {
    showToast("需要在 Tauri 应用里运行才能切换主题", true);
    return;
  }
  applying = true;
  try {
    const message = await applyTheme(theme.name);
    current.value = theme.name;
    themes.value = themes.value.map((item) => ({
      ...item,
      is_current: item.name === theme.name,
    }));
    showToast(message || "已切换");
  } catch (err) {
    showToast("切换失败：" + errorText(err), true);
  } finally {
    applying = false;
  }
}

/* ---------- 体检 ---------- */
const pendingCount = computed(
  () =>
    health.value.filter((item) => item.level === "warn" || item.level === "error")
      .length
);

async function loadHealth() {
  try {
    health.value = await checkHealth();
  } catch (err) {
    showToast("体检失败：" + errorText(err), true);
  }
}

async function onRunFix(item: CheckItem) {
  if (busy.value) return;
  setBusy(true);
  try {
    const message = await runFix(item.id);
    appendFixLog(`【${item.title}】\n${message}`);
    showToast("已处理：" + item.title);
    await Promise.all([loadHealth(), loadThemes()]);
  } catch (err) {
    appendFixLog(`【${item.title}】\n失败：${errorText(err)}`);
    showToast("处理失败：" + errorText(err), true);
  } finally {
    setBusy(false);
  }
}

async function onRunAll() {
  if (busy.value) return;
  setBusy(true);
  try {
    const message = await runAllFixes();
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
  try {
    autostart.value = await getAutostart();
  } catch {
    /* ignore */
  }
}

async function onToggleAutostart(event: Event) {
  const checked = (event.target as HTMLInputElement).checked;
  try {
    const message = await setAutostart(checked);
    showToast(message);
    void loadHealth();
  } catch (err) {
    autostart.value = !checked;
    showToast("设置失败：" + errorText(err), true);
  }
}

async function onRegisterShortcut() {
  try {
    const message = await registerShortcut(shortcutAccel.value.trim());
    showToast(message);
    void loadHealth();
  } catch (err) {
    showToast(errorText(err), true);
    void loadHealth();
  }
}

async function onOpenConfig() {
  try {
    await openFcitxConfig();
  } catch (err) {
    showToast("打开失败：" + errorText(err), true);
  }
}

/* ---------- 启动 ---------- */
onMounted(async () => {
  void loadThemes();
  void loadHealth();
  void loadAutostart();
  try {
    info.value = await appInfo();
    if (info.value?.shortcut) shortcutAccel.value = info.value.shortcut;
  } catch {
    /* ignore */
  }
  if (!hasTauri()) {
    showToast("浏览器预览模式：切换/修复功能需要在 Tauri 应用内运行", true);
  }
});

const footerMeta = computed(() => {
  const version = info.value?.version ?? "0.3.0";
  const tested = info.value?.tested_on ?? "CachyOS";
  return `v${version} · 仅在 ${tested} 测试过`;
});
</script>

<template>
  <header class="top">
    <div class="titles">
      <h1>输入法候选框样式</h1>
      <p class="sub">fcitx5 候选框主题切换 · 体检与初始化</p>
    </div>
    <div class="actions">
      <input
        v-model="keyword"
        class="search"
        type="search"
        placeholder="搜索主题 / 作者…"
        autocomplete="off"
      />
      <button class="btn" title="重新扫描主题目录" @click="loadThemes">
        刷新
      </button>
    </div>
  </header>

  <nav class="tabs">
    <button
      v-for="tab in tabs"
      :key="tab.id"
      class="tab"
      :class="{ active: activeTab === tab.id }"
      @click="activeTab = tab.id"
    >
      {{ tab.label }}
      <span
        v-if="tab.id === 'health' && pendingCount > 0"
        class="tab-badge"
      >{{ pendingCount }}</span>
    </button>
    <span class="spacer" />
    <span class="current-chip">当前：<b>{{ currentDisplay }}</b></span>
  </nav>

  <main>
    <!-- 主题 -->
    <section v-show="activeTab === 'themes'" class="view">
      <div class="meta-row">
        <span class="muted">{{ countText }}</span>
        <span class="muted hint">主题 = fcitx5「经典界面」皮肤，rime / 拼音通用</span>
      </div>
      <section class="grid">
        <div v-if="visibleThemes.length === 0" class="empty">
          {{ keyword.trim() ? "没有匹配的主题" : "没有扫描到主题目录" }}
        </div>
        <ThemeCard
          v-for="theme in visibleThemes"
          :key="theme.name"
          :theme="theme"
          @select="onSelectTheme"
        />
      </section>
    </section>

    <!-- 体检 / 初始化 -->
    <section v-show="activeTab === 'health'" class="view">
      <div class="health-toolbar">
        <button class="btn primary" :disabled="busy" @click="onRunAll">
          一键初始化 / 修复
        </button>
        <button class="btn" :disabled="busy" @click="loadHealth">重新体检</button>
        <label class="switch" title="开机后静默启动到托盘">
          <input
            :checked="autostart"
            type="checkbox"
            @change="onToggleAutostart"
          />
          开机自启
        </label>
        <span class="shortcut-box">
          <input v-model="shortcutAccel" class="search shortcut-input" />
          <button class="btn ghost" :disabled="busy" @click="onRegisterShortcut">
            注册全局快捷键
          </button>
        </span>
        <span class="spacer" />
        <button class="btn ghost" @click="onOpenConfig">fcitx5 设置</button>
      </div>

      <div class="note">
        仅在 <b>CachyOS（Arch 系）</b> 上测试过，其他发行版请自行确认依赖：
        <code>webkit2gtk-4.1</code>、<code>gtk3</code>、<code>librsvg</code>、<code>libayatana-appindicator</code>。
        Wayland 下全局快捷键通常不可用，建议在 KDE「系统设置 → 快捷键」里给本应用绑定。
      </div>

      <div class="health-list">
        <div
          v-for="item in health"
          :key="item.id"
          class="check"
          :class="item.level"
        >
          <span class="dot" />
          <div class="body">
            <div class="title">
              {{ item.title }}
              <span class="tag">{{ { ok: "正常", warn: "待处理", error: "错误", info: "说明" }[item.level] }}</span>
            </div>
            <div class="detail">{{ item.detail }}</div>
          </div>
          <div v-if="item.fixable" class="fix">
            <button class="btn" :disabled="busy" @click="onRunFix(item)">
              {{ item.fix_label || "修复" }}
            </button>
          </div>
        </div>
      </div>

      <pre v-if="showFixLog" class="fix-log">{{ fixLog }}</pre>
    </section>
  </main>

  <footer class="foot">
    <span>{{ footerMeta }}</span>
    <span class="foot-sep">·</span>
    主题目录：<code>~/.local/share/fcitx5/themes</code> 与
    <code>/usr/share/fcitx5/themes</code>
    <span class="foot-sep">·</span>
    配置写入 <code>~/.config/fcitx5/conf/classicui.conf</code>
  </footer>

  <div class="toast" :class="{ show: toastVisible, error: toastError }">
    {{ toastText }}
  </div>
</template>
