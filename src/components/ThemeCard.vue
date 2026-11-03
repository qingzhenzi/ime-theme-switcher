<script setup lang="ts">
import { computed, ref } from "vue";
import type { ThemeInfo } from "../api";
import { toCssColor } from "../colors";

const props = defineProps<{ theme: ThemeInfo }>();
const emit = defineEmits<{ (e: "select", theme: ThemeInfo): void }>();

const previewFailed = ref(false);

const previewSrc = computed(
  () => `previews/${encodeURIComponent(props.theme.name)}.png`
);

const displayName = computed(
  () => props.theme.display_name || props.theme.name
);

const showId = computed(
  () => props.theme.display_name && props.theme.display_name !== props.theme.name
);

const title = computed(
  () =>
    `点击切换为「${displayName.value}」` +
    (showId.value ? `（${props.theme.name}）` : "")
);

const swatchStyle = computed(() => ({
  background: toCssColor(props.theme.background_color, "rgba(255, 255, 255, 0.06)"),
  border: `1px solid ${toCssColor(props.theme.border_color, "rgba(255, 255, 255, 0.18)")}`,
  color: toCssColor(props.theme.normal_color, "#d8dee9"),
}));

const highlightStyle = computed(() => ({
  background: toCssColor(props.theme.highlight_color, "rgba(136, 192, 208, 0.35)"),
  color: toCssColor(props.theme.highlight_text_color, "#102a32"),
}));
</script>

<template>
  <button
    type="button"
    class="card"
    :class="{ current: theme.is_current }"
    :title="title"
    @click="emit('select', theme)"
  >
    <div class="preview" :class="{ failed: previewFailed }">
      <img
        v-if="!previewFailed"
        :src="previewSrc"
        alt=""
        loading="lazy"
        @error="previewFailed = true"
      />
      <div v-else class="swatch">
        <div class="swatch-pill" :style="swatchStyle">
          <span class="hl" :style="highlightStyle">1 你好</span>
          <span>2 世界 3 输入</span>
        </div>
      </div>
    </div>

    <div class="info">
      <div class="name-row">
        <span class="name">{{ displayName }}</span>
        <span v-if="showId" class="id">{{ theme.name }}</span>
      </div>
      <div class="sub-row">
        <span v-if="theme.is_current" class="badge now">使用中</span>
        <span class="badge" :class="theme.source === 'user' ? 'user' : 'system'">
          {{ theme.source === "user" ? "用户主题" : "系统主题" }}
        </span>
        <span v-if="!theme.has_page_buttons" class="badge nopage">无翻页按钮</span>
        <span v-if="theme.author" class="author">by {{ theme.author }}</span>
      </div>
    </div>
  </button>
</template>
