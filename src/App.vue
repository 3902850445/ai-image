<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  callEdit,
  callGenerate,
  computeSize,
  DEFAULT_SETTINGS,
  imageSize,
  loadSettings,
  MODEL_NAME,
  pickDirectory,
  RATIOS,
  saveImageFile,
  saveSettings,
  TIERS,
  timestampName,
  type Provider,
  type Settings,
} from "./lib/api";

// ---------------- 状态 ----------------
const APP_VERSION = "v0.2.0";

const settings = ref<Settings>(structuredClone(DEFAULT_SETTINGS));
const settingsLoaded = ref(false);

const prompt = ref("");
const count = ref(1);
const ratio = ref("auto");
const pixelTier = ref("1k");
const quality = ref("medium");

interface RefImage {
  id: number;
  name: string;
  b64: string;
  dataUrl: string;
}
const refImages = ref<RefImage[]>([]);
let refIdSeed = 1;

interface GenItem {
  id: number;
  b64: string;
  mime: string;
  dataUrl: string;
  prompt: string;
  w: number;
  h: number;
  durationMs: number;
  saving: boolean;
}
const results = ref<GenItem[]>([]);
const pendingCount = ref(0);
const doneCount = ref(0);
const totalCount = ref(0);
let genIdSeed = 1;

const generating = ref(false);
let stopFlag = false;
const failCount = ref(0);

interface LogEntry {
  time: string;
  text: string;
  level: "info" | "error";
}
const logs = ref<LogEntry[]>([]);

const toast = ref<{ text: string; ok: boolean } | null>(null);
let toastTimer: ReturnType<typeof setTimeout> | null = null;

// 设置弹窗
const showSettings = ref(false);
const editSettings = ref<Settings>(structuredClone(DEFAULT_SETTINGS));
const logPanelOpen = ref(false);
const refInput = ref<HTMLInputElement | null>(null);

function onRefInputChange(e: Event) {
  const target = e.target as HTMLInputElement;
  if (target.files && target.files.length > 0) {
    addImageFiles(target.files);
  }
  target.value = "";
}

// ---------------- 基础工具 ----------------
function nowTime(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

function log(text: string, level: "info" | "error" = "info") {
  logs.value.unshift({ time: nowTime(), text, level });
  if (logs.value.length > 100) logs.value.pop();
}

function showToast(text: string, ok: boolean) {
  toast.value = { text, ok };
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (toast.value = null), ok ? 2200 : 4200);
}

// ---------------- 计算属性 ----------------
const activeProvider = computed<Provider | null>(() => {
  const s = settings.value;
  return s.providers.find((p) => p.id === s.activeProviderId) ?? s.providers[0] ?? null;
});

const hasApiKey = computed(() => !!activeProvider.value && activeProvider.value.apiKey.trim() !== "");

const outputSizeLabel = computed(() => computeSize(ratio.value, pixelTier.value));

const qualityLabel = computed(() => {
  const map: Record<string, string> = { low: "低", medium: "中", high: "高" };
  return map[quality.value] ?? quality.value;
});

// ---------------- 初始化 ----------------
onMounted(async () => {
  settings.value = await loadSettings();
  settingsLoaded.value = true;
  log(`应用启动完成，共 ${settings.value.providers.length} 个供应商配置`);
  window.addEventListener("paste", onPaste);
});

// ---------------- 参考图 ----------------
const MAX_REFS = 16;

async function addImageFiles(files: FileList | File[]) {
  for (const file of Array.from(files)) {
    if (!file.type.startsWith("image/")) continue;
    if (refImages.value.length >= MAX_REFS) {
      showToast(`参考图最多 ${MAX_REFS} 张`, false);
      break;
    }
    const dataUrl = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = () => reject(new Error("读取图片失败"));
      reader.readAsDataURL(file);
    });
    const b64 = dataUrl.slice(dataUrl.indexOf(",") + 1);
    refImages.value.push({ id: refIdSeed++, name: file.name || "粘贴图片", b64, dataUrl });
  }
  log(`当前参考图 ${refImages.value.length}/${MAX_REFS} 张`);
}

function removeRef(id: number) {
  refImages.value = refImages.value.filter((r) => r.id !== id);
}

function clearRefs() {
  refImages.value = [];
}

function onPaste(e: ClipboardEvent) {
  if (showSettings.value) return;
  const items = e.clipboardData?.items;
  if (!items) return;
  const files: File[] = [];
  for (const item of items) {
    if (item.kind === "file" && item.type.startsWith("image/")) {
      const f = item.getAsFile();
      if (f) files.push(f);
    }
  }
  if (files.length > 0) {
    e.preventDefault();
    addImageFiles(files);
  }
}

function onDrop(e: DragEvent) {
  const files = e.dataTransfer?.files;
  if (files && files.length > 0) {
    addImageFiles(files);
  }
}

// ---------------- 生成 ----------------
function stopGenerate() {
  stopFlag = true;
  log("已请求停止，正在等待进行中的任务结束…");
}

async function runOne(): Promise<void> {
  const provider = activeProvider.value;
  if (!provider) return;
  const start = performance.now();
  const useEdit = refImages.value.length > 0;
  try {
    const result = useEdit
      ? await callEdit({
          baseUrl: provider.baseUrl,
          apiKey: provider.apiKey,
          prompt: prompt.value,
          size: outputSizeLabel.value,
          quality: quality.value,
          imagesB64: refImages.value.map((r) => r.b64),
          responseFormat: provider.supportsResponseFormat ? "b64_json" : null,
        })
      : await callGenerate({
          baseUrl: provider.baseUrl,
          apiKey: provider.apiKey,
          prompt: prompt.value,
          size: outputSizeLabel.value,
          quality: quality.value,
          responseFormat: provider.supportsResponseFormat ? "b64_json" : null,
        });
    const durationMs = Math.round(performance.now() - start);
    const { w, h } = await imageSize(result.image_b64, result.mime);
    results.value.unshift({
      id: genIdSeed++,
      b64: result.image_b64,
      mime: result.mime,
      dataUrl: `data:${result.mime};base64,${result.image_b64}`,
      prompt: prompt.value,
      w,
      h,
      durationMs,
      saving: false,
    });
    doneCount.value++;
    log(`生成成功（${(durationMs / 1000).toFixed(1)}s，${w}x${h}）`);
  } catch (err) {
    doneCount.value++;
    failCount.value++;
    const msg = err instanceof Error ? err.message : String(err);
    log(`生成失败：${msg}`, "error");
    showToast(`生成失败：${msg}`, false);
  }
}

async function generate() {
  if (generating.value) return;
  if (!activeProvider.value) {
    showToast("请先在设置中添加供应商", false);
    return;
  }
  if (!hasApiKey.value) {
    showToast("请先在设置中配置 API Key", false);
    return;
  }
  if (prompt.value.trim() === "") {
    showToast("请先输入提示词", false);
    return;
  }

  generating.value = true;
  stopFlag = false;
  doneCount.value = 0;
  failCount.value = 0;
  totalCount.value = count.value;
  pendingCount.value = count.value;
  log(
    `开始生成 ${count.value} 张（${MODEL_NAME}，${outputSizeLabel.value}，质量：${qualityLabel.value}，` +
      `${useEditText.value ? `图生图 ${refImages.value.length} 张参考图` : "文生图"}，并发 ${Math.min(activeProvider.value.maxConcurrency, count.value)}）`,
  );

  let next = 0;
  const total = count.value;
  const workers = Array.from(
    { length: Math.max(1, Math.min(activeProvider.value.maxConcurrency, total)) },
    async () => {
      while (!stopFlag) {
        const my = next++;
        if (my >= total) break;
        await runOne();
        pendingCount.value = Math.max(0, pendingCount.value - 1);
      }
    },
  );
  await Promise.all(workers);

  generating.value = false;
  pendingCount.value = 0;
  const success = doneCount.value - failCount.value;
  if (stopFlag) {
    log(`已停止：成功 ${success}/${total} 张`);
  } else {
    log(`本轮生成结束：成功 ${success}/${total} 张`);
  }
}

const useEditText = computed(() => refImages.value.length > 0);

// ---------------- 下载保存 ----------------
async function ensureSaveDir(): Promise<string | null> {
  if (settings.value.saveDir) return settings.value.saveDir;
  const picked = await pickDirectory();
  if (!picked) return null;
  settings.value.saveDir = picked;
  await saveSettings(settings.value);
  log(`保存路径已设为：${picked}`);
  return picked;
}

async function downloadOne(item: GenItem) {
  const dir = await ensureSaveDir();
  if (!dir) return;
  item.saving = true;
  try {
    const filename = `${timestampName()}.${settings.value.saveFormat}`;
    const path = await saveImageFile({
      b64: item.b64,
      mime: item.mime,
      format: settings.value.saveFormat,
      jpgQuality: settings.value.jpgQuality,
      dir,
      filename,
    });
    log(`已保存：${path}`);
    showToast("图片已保存 ✓", true);
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    log(`保存失败：${msg}`, "error");
    showToast(`保存失败：${msg}`, false);
  } finally {
    item.saving = false;
  }
}

const bulkSaving = ref(false);

async function downloadAll() {
  if (results.value.length === 0 || bulkSaving.value) return;
  const dir = await ensureSaveDir();
  if (!dir) return;
  bulkSaving.value = true;
  let ok = 0;
  let fail = 0;
  for (const [i, item] of results.value.entries()) {
    item.saving = true;
    try {
      const filename = `${timestampName(`ai-image-${i + 1}`)}.${settings.value.saveFormat}`;
      const path = await saveImageFile({
        b64: item.b64,
        mime: item.mime,
        format: settings.value.saveFormat,
        jpgQuality: settings.value.jpgQuality,
        dir,
        filename,
      });
      ok++;
      log(`已保存（${i + 1}/${results.value.length}）：${path}`);
    } catch (err) {
      fail++;
      const msg = err instanceof Error ? err.message : String(err);
      log(`保存失败（${i + 1}/${results.value.length}）：${msg}`, "error");
    } finally {
      item.saving = false;
    }
  }
  bulkSaving.value = false;
  showToast(`批量保存完成：成功 ${ok} 张${fail > 0 ? `，失败 ${fail} 张` : ""}`, fail === 0);
}

async function copyImage(item: GenItem) {
  try {
    const bin = atob(item.b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    const blob = new Blob([bytes], { type: "image/png" });
    await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
    showToast("已复制到剪贴板 ✓", true);
    log("图片已复制到剪贴板");
  } catch {
    showToast("复制失败：当前环境不支持剪贴板图片", false);
    log("复制到剪贴板失败", "error");
  }
}

// ---------------- 设置 ----------------
function openSettings() {
  editSettings.value = JSON.parse(JSON.stringify(settings.value));
  showSettings.value = true;
}

function addProvider() {
  const id = `custom_${Date.now()}`;
  editSettings.value.providers.push({
    id,
    name: "自定义供应商",
    baseUrl: "https://",
    apiKey: "",
    supportsResponseFormat: true,
    maxConcurrency: 2,
  });
  editSettings.value.activeProviderId = id;
}

function removeProvider(id: string) {
  const list = editSettings.value.providers;
  if (list.length <= 1) {
    showToast("至少保留一个供应商", false);
    return;
  }
  editSettings.value.providers = list.filter((p) => p.id !== id);
  if (editSettings.value.activeProviderId === id) {
    editSettings.value.activeProviderId = editSettings.value.providers[0].id;
  }
}

async function chooseSaveDir() {
  const picked = await pickDirectory();
  if (picked) editSettings.value.saveDir = picked;
}

async function applySettings() {
  const s = editSettings.value;
  const active = s.providers.find((p) => p.id === s.activeProviderId);
  if (active && active.baseUrl.trim() === "") {
    showToast("当前供应商的 API 地址不能为空", false);
    return;
  }
  for (const p of s.providers) {
    p.maxConcurrency = Math.max(1, Math.min(10, Math.round(p.maxConcurrency) || 1));
    p.name = p.name.trim() || "未命名供应商";
    p.baseUrl = p.baseUrl.trim();
  }
  settings.value = JSON.parse(JSON.stringify(s));
  await saveSettings(settings.value);
  showSettings.value = false;
  log(
    `设置已保存：激活供应商 ${active?.name ?? "-"}，保存格式 ${settings.value.saveFormat.toUpperCase()}，` +
      `保存目录 ${settings.value.saveDir || "未设置（首次保存时选择）"}`,
  );
  showToast("设置已保存 ✓", true);
}

// 监听格式变化写日志
watch(
  () => settings.value.saveFormat,
  (fmt, oldFmt) => {
    if (settingsLoaded.value && oldFmt && fmt !== oldFmt) {
      log(`保存格式切换为 ${fmt.toUpperCase()}`);
    }
  },
);
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 顶栏 -->
    <nav
      class="flex h-12 shrink-0 items-center border-b px-4"
      style="background: #fff; border-color: rgb(0 0 0 / 0.08)"
    >
      <div class="flex items-center gap-2">
        <div
          class="flex h-6 w-6 items-center justify-center rounded"
          style="background: #346aea"
        >
          <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="none" stroke="#fff" stroke-width="2">
            <rect x="3" y="3" width="18" height="18" rx="3" />
            <circle cx="9" cy="9" r="2" />
            <path d="m21 15-3.5-3.5L7 22" />
          </svg>
        </div>
        <span class="text-[13px] font-semibold" style="color: #1a1a1a">AI 图片生成</span>
        <span class="ml-1 text-[11px] font-normal" style="color: #919191">{{ APP_VERSION }}</span>
      </div>
      <div class="ml-auto flex items-center gap-2">
        <span
          v-if="activeProvider"
          class="rounded-full px-2.5 py-1 text-[11px]"
          style="background: rgb(52 106 234 / 0.08); color: #346aea"
        >
          {{ activeProvider.name }} · {{ MODEL_NAME }}
        </span>
        <button
          class="flex h-8 items-center gap-1.5 rounded-lg px-3 text-xs transition-colors hover:bg-black/10"
          style="background: rgb(0 0 0 / 0.04); color: #1a1a1a"
          @click="openSettings"
        >
          <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
          </svg>
          设置
        </button>
      </div>
    </nav>

    <div class="flex min-h-0 flex-1 overflow-hidden" style="background: #f5f5f5">
      <!-- 左侧参数栏 -->
      <aside
        class="flex w-[360px] shrink-0 flex-col overflow-hidden border-r"
        style="background: #fff; border-color: rgb(0 0 0 / 0.1)"
      >
        <div class="custom-scrollbar min-h-0 flex-1 overflow-y-auto px-5 pb-3 pt-5">
          <!-- 参考图片 -->
          <div class="mb-5" @drop.prevent="onDrop" @dragover.prevent>
            <div class="mb-3 flex items-center justify-between">
              <span class="text-sm font-medium" style="color: #1a1a1a">
                参考图片<span class="ml-1 text-[10px] font-normal" style="color: #3c3a3a">（拖拽或粘贴可添加）</span>
              </span>
              <span class="text-xs" style="color: #616161">{{ refImages.length }}/{{ MAX_REFS }}</span>
            </div>
            <div class="flex flex-wrap gap-2">
              <div
                v-for="img in refImages"
                :key="img.id"
                class="group relative h-20 w-20 overflow-hidden rounded-xl border"
                style="border-color: rgb(0 0 0 / 0.15)"
              >
                <img :src="img.dataUrl" class="h-full w-full object-cover" :alt="img.name" />
                <button
                  class="absolute right-1 top-1 flex h-5 w-5 items-center justify-center rounded-full text-[10px] text-white opacity-0 transition-opacity group-hover:opacity-100"
                  style="background: rgb(0 0 0 / 0.55)"
                  title="移除"
                  @click="removeRef(img.id)"
                >
                  ✕
                </button>
              </div>
              <button
                v-if="refImages.length < MAX_REFS"
                class="flex h-20 w-20 items-center justify-center gap-1.5 rounded-xl border border-dashed text-xs transition-all duration-200 hover:border-[#346aea] hover:text-[#346aea]"
                style="border-color: rgb(0 0 0 / 0.15); color: #616161; background: rgb(0 0 0 / 0.03)"
                @click="refInput?.click()"
              >
                <svg viewBox="0 0 24 24" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M12 5v14M5 12h14" />
                </svg>
                添加
              </button>
            </div>
            <input
              ref="refInput"
              type="file"
              accept="image/*"
              multiple
              class="hidden"
              @change="onRefInputChange"
            />
            <p v-if="refImages.length > 0" class="mt-1.5">
              <button class="text-[10px] hover:underline" style="color: #919191" @click="clearRefs">
                清空全部参考图
              </button>
            </p>
          </div>

          <!-- 提示词 -->
          <div class="mb-4">
            <label class="mb-2.5 block text-sm font-medium" style="color: #1a1a1a">提示词</label>
            <div
              class="overflow-hidden rounded-xl border transition-colors duration-200 focus-within:border-[#346aea] hover:border-black/20"
              style="background: rgb(0 0 0 / 0.04); border-color: rgb(0 0 0 / 0.15)"
            >
              <textarea
                v-model="prompt"
                placeholder="描述你想生成的图片…"
                class="h-36 w-full resize-y bg-transparent px-4 py-3 text-sm outline-none focus:ring-2 focus:ring-[#346aea]/20"
                style="color: #1a1a1a"
              ></textarea>
            </div>
          </div>

          <!-- 数量 / 比例 / 档位 -->
          <div class="mb-4 flex flex-wrap items-center gap-2">
            <div
              class="flex h-10 items-center overflow-hidden rounded-lg"
              style="background: rgb(0 0 0 / 0.04)"
              title="生成数量"
            >
              <button
                class="flex h-10 w-9 items-center justify-center text-sm transition-colors duration-150 hover:bg-black/10"
                style="color: #616161"
                :disabled="count <= 1"
                @click="count = Math.max(1, count - 1)"
              >
                −
              </button>
              <span class="w-7 text-center text-sm font-semibold" style="color: #1a1a1a">{{ count }}</span>
              <button
                class="flex h-10 w-9 items-center justify-center text-sm transition-colors duration-150 hover:bg-black/10"
                style="color: #616161"
                :disabled="count >= 10"
                @click="count = Math.min(10, count + 1)"
              >
                +
              </button>
            </div>
            <select
              v-model="ratio"
              class="h-10 flex-1 rounded-lg px-3 text-xs outline-none"
              style="background: rgb(0 0 0 / 0.04); color: #1a1a1a"
            >
              <option v-for="r in RATIOS" :key="r.value" :value="r.value">比例 {{ r.label }}</option>
            </select>
            <div
              class="relative flex h-10 shrink-0 items-center overflow-hidden rounded-lg p-0.5"
              style="width: 8rem; background: rgb(0 0 0 / 0.04)"
            >
              <button
                v-for="t in TIERS"
                :key="t.value"
                class="relative h-full flex-1 rounded-md text-xs font-medium transition-colors duration-150"
                :style="pixelTier === t.value ? 'background: rgb(0 0 0 / 0.1); color: #1a1a1a' : 'color: #919191'"
                :title="t.estimate"
                @click="pixelTier = t.value"
              >
                {{ t.label }}
              </button>
            </div>
          </div>

          <!-- 质量 -->
          <div class="mb-4">
            <label class="mb-2.5 block text-sm font-medium" style="color: #1a1a1a">质量</label>
            <div class="relative flex gap-1 overflow-hidden rounded-lg p-0.5" style="background: rgb(0 0 0 / 0.04)">
              <div
                class="absolute inset-y-0.5 rounded-md transition-all duration-300 ease-out"
                :style="{
                  left: `calc(${['low', 'medium', 'high'].indexOf(quality)} * 100% / 3 + 2px)`,
                  width: 'calc(100% / 3 - 4px)',
                  background: '#1a1a1a',
                }"
              ></div>
              <button
                v-for="q in ['low', 'medium', 'high']"
                :key="q"
                class="relative h-8 flex-1 rounded-md text-xs font-medium transition-colors duration-150"
                :style="{ color: quality === q ? '#fff' : '#616161' }"
                @click="quality = q"
              >
                {{ { low: '低', medium: '中', high: '高' }[q as 'low' | 'medium' | 'high'] }}
              </button>
            </div>
            <p class="mt-2 text-[10px] leading-relaxed" style="color: #3c3a3a">
              提示词越长、分辨率越高、质量越高，生成等待时间都会更长
            </p>
          </div>

          <!-- 运行日志（内嵌简版） -->
          <div v-if="logs.length > 0" class="mb-2">
            <button
              class="mb-1.5 flex items-center gap-1 text-[10px] hover:underline"
              style="color: #919191"
              @click="logPanelOpen = !logPanelOpen"
            >
              运行日志（{{ logs.length }}）{{ logPanelOpen ? '▲' : '▼' }}
            </button>
            <div
              v-if="logPanelOpen"
              class="custom-scrollbar max-h-40 overflow-y-auto rounded-lg p-2"
              style="background: rgb(0 0 0 / 0.03)"
            >
              <p
                v-for="(l, i) in logs.slice(0, 30)"
                :key="i"
                class="text-[10px] leading-relaxed"
                :style="{ color: l.level === 'error' ? '#d3482b' : '#616161' }"
              >
                [{{ l.time }}] {{ l.text }}
              </p>
            </div>
          </div>
        </div>

        <!-- 底部状态 + 生成按钮 -->
        <div
          class="shrink-0 space-y-3 border-t p-4"
          style="border-color: rgb(0 0 0 / 0.1); background: #fff"
        >
          <div class="flex items-center gap-2 text-xs" style="color: #616161">
            <span class="truncate">{{ activeProvider?.name ?? '未配置供应商' }} · {{ MODEL_NAME }}</span>
            <span class="ml-auto shrink-0 text-[10px]" style="color: #919191">
              {{ generating ? `生成中 ${doneCount}/${totalCount}` : `并发 ${activeProvider?.maxConcurrency ?? 1} · ${outputSizeLabel}` }}
            </span>
            <button
              v-if="generating"
              class="flex h-8 w-8 items-center justify-center rounded-lg transition-colors duration-150"
              style="color: #d3482b; background: rgb(211 72 43 / 0.1); border: 1px solid rgb(211 72 43 / 0.3)"
              title="停止生成"
              @click="stopGenerate"
            >
              <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="currentColor">
                <rect x="6" y="6" width="12" height="12" rx="2" />
              </svg>
            </button>
          </div>
          <button
            class="flex h-12 w-full items-center justify-center gap-2 rounded-xl text-sm font-semibold transition-all duration-200 active:scale-[0.97]"
            :disabled="generating || !hasApiKey"
            :style="
              generating || !hasApiKey
                ? 'background: rgb(0 0 0 / 0.04); color: #919191; cursor: not-allowed'
                : 'background: #346aea; color: #fff'
            "
            @click="generate"
          >
            <svg v-if="!generating" viewBox="0 0 24 24" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2">
              <path d="m12 3-1.9 5.8a2 2 0 0 1-1.3 1.3L3 12l5.8 1.9a2 2 0 0 1 1.3 1.3L12 21l1.9-5.8a2 2 0 0 1 1.3-1.3L21 12l-5.8-1.9a2 2 0 0 1-1.3-1.3z" />
            </svg>
            <span v-if="generating">生成中…（点击右侧停止按钮可中断）</span>
            <span v-else-if="!hasApiKey">请先配置 API Key</span>
            <span v-else>{{ useEditText ? '开始生成（图生图）' : '开始生成' }}</span>
          </button>
          <p class="text-center text-[10px]" style="color: #919191">
            本地应用，密钥与配置仅保存在本机
          </p>
        </div>
      </aside>

      <!-- 结果区 -->
      <main class="relative flex min-w-0 flex-1 flex-col" style="background: #f5f5f5">
        <div class="custom-scrollbar min-h-0 flex-1 overflow-y-auto p-6">
          <!-- 空状态 -->
          <div
            v-if="results.length === 0 && pendingCount === 0"
            class="flex h-full flex-col items-center justify-center gap-4 text-center"
            style="color: #616161"
          >
            <div class="flex h-16 w-16 items-center justify-center rounded-2xl" style="background: rgb(0 0 0 / 0.04)">
              <svg viewBox="0 0 24 24" class="h-7 w-7" fill="none" stroke="#919191" stroke-width="1.5">
                <rect x="3" y="3" width="18" height="18" rx="3" />
                <circle cx="9" cy="9" r="2" />
                <path d="m21 15-3.5-3.5L7 22" />
              </svg>
            </div>
            <p class="text-sm">生成结果会显示在这里</p>
          </div>

          <!-- 结果网格 -->
          <div class="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-4">
            <!-- 生成中占位 -->
            <div
              v-for="i in pendingCount"
              :key="`pending-${i}`"
              class="flex aspect-square animate-pulse items-center justify-center rounded-2xl border"
              style="background: rgb(0 0 0 / 0.03); border-color: rgb(0 0 0 / 0.08)"
            >
              <div class="text-center">
                <div
                  class="mx-auto mb-2 h-8 w-8 animate-spin rounded-full border-2 border-t-transparent"
                  style="border-color: #346aea; border-top-color: transparent"
                ></div>
                <p class="text-xs" style="color: #919191">生成中…</p>
              </div>
            </div>
            <!-- 已完成图片 -->
            <div
              v-for="item in results"
              :key="item.id"
              class="group overflow-hidden rounded-2xl border"
              style="background: #fff; border-color: rgb(0 0 0 / 0.1)"
            >
              <div class="relative">
                <img :src="item.dataUrl" class="w-full" :style="{ aspectRatio: item.w && item.h ? `${item.w}/${item.h}` : '1/1', objectFit: 'contain' }" />
                <div
                  class="absolute inset-x-0 bottom-0 flex items-center justify-end gap-1.5 p-2 opacity-0 transition-opacity group-hover:opacity-100"
                  style="background: linear-gradient(to top, rgb(0 0 0 / 0.45), transparent)"
                >
                  <button
                    class="flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-xs font-medium transition-colors"
                    style="background: rgb(255 255 255 / 0.92); color: #1a1a1a"
                    title="复制图片"
                    @click="copyImage(item)"
                  >
                    复制
                  </button>
                  <button
                    class="flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-xs font-medium transition-colors"
                    :disabled="item.saving"
                    :style="item.saving ? 'background: rgb(255 255 255 / 0.6); color: #919191' : 'background: #346aea; color: #fff'"
                    title="保存图片"
                    @click="downloadOne(item)"
                  >
                    {{ item.saving ? '保存中…' : `下载 ${settings.saveFormat.toUpperCase()}` }}
                  </button>
                </div>
              </div>
              <div class="flex items-start gap-2 px-3 py-2">
                <p class="line-clamp-2 flex-1 text-[11px] leading-relaxed" style="color: #616161" :title="item.prompt">
                  {{ item.prompt }}
                </p>
                <span class="shrink-0 text-[10px]" style="color: #919191">
                  {{ item.w }}×{{ item.h }} · {{ (item.durationMs / 1000).toFixed(1) }}s
                </span>
              </div>
            </div>
          </div>
        </div>

        <!-- 批量保存浮条 -->
        <div
          v-if="results.length > 1"
          class="pointer-events-auto absolute bottom-4 left-1/2 flex -translate-x-1/2 items-center gap-3 rounded-full px-4 py-2 shadow-lg"
          style="background: rgb(255 255 255 / 0.95); border: 1px solid rgb(0 0 0 / 0.1)"
        >
          <span class="text-xs" style="color: #616161">
            共 {{ results.length }} 张 · 保存格式 {{ settings.saveFormat.toUpperCase() }}
            · {{ settings.saveDir ? '已设保存路径' : '未设保存路径' }}
          </span>
          <button
            class="flex h-8 items-center gap-1.5 rounded-full px-3.5 text-xs font-medium transition-all active:scale-95"
            :disabled="bulkSaving"
            :style="bulkSaving ? 'background: rgb(0 0 0 / 0.06); color: #919191' : 'background: #346aea; color: #fff'"
            @click="downloadAll"
          >
            {{ bulkSaving ? '批量保存中…' : '全部下载' }}
          </button>
        </div>

        <span
          class="pointer-events-none absolute bottom-3 right-4 z-30 select-none text-[10px]"
          style="color: rgb(0 0 0 / 0.15)"
        >
          ai-image · Tauri 2 本地复刻版
        </span>
      </main>
    </div>

    <!-- 设置弹窗 -->
    <Teleport to="body">
      <Transition name="modal">
        <div
          v-if="showSettings"
          class="fixed inset-0 z-50 flex items-center justify-center p-6"
          style="background: rgb(0 0 0 / 0.4)"
          @click.self="showSettings = false"
        >
          <div
            class="custom-scrollbar max-h-[86vh] w-full max-w-2xl overflow-y-auto rounded-2xl p-6"
            style="background: #fff"
          >
            <div class="mb-5 flex items-center justify-between">
              <h2 class="text-base font-semibold" style="color: #1a1a1a">设置</h2>
              <button
                class="flex h-8 w-8 items-center justify-center rounded-lg text-sm hover:bg-black/10"
                style="color: #616161"
                @click="showSettings = false"
              >
                ✕
              </button>
            </div>

            <!-- 供应商管理 -->
            <h3 class="mb-3 text-sm font-medium" style="color: #1a1a1a">API 供应商</h3>
            <div class="mb-2 space-y-3">
              <div
                v-for="(p, idx) in editSettings.providers"
                :key="p.id"
                class="rounded-xl border p-4"
                :style="
                  editSettings.activeProviderId === p.id
                    ? 'border-color: #346aea; background: rgb(52 106 234 / 0.04)'
                    : 'border-color: rgb(0 0 0 / 0.12)'
                "
              >
                <div class="mb-3 flex items-center gap-2">
                  <label class="flex cursor-pointer items-center gap-1.5 text-xs" style="color: #1a1a1a">
                    <input
                      type="radio"
                      :name="'active-provider'"
                      :checked="editSettings.activeProviderId === p.id"
                      @change="editSettings.activeProviderId = p.id"
                    />
                    使用此供应商
                  </label>
                  <span class="ml-auto text-[10px]" style="color: #919191">供应商 {{ idx + 1 }}</span>
                  <button
                    class="flex h-7 items-center rounded-lg px-2 text-[11px] hover:opacity-80"
                    style="color: #d3482b; background: rgb(211 72 43 / 0.08)"
                    @click="removeProvider(p.id)"
                  >
                    删除
                  </button>
                </div>
                <div class="grid grid-cols-2 gap-3">
                  <label class="block text-xs" style="color: #616161">
                    名称
                    <input
                      v-model="p.name"
                      class="mt-1 h-9 w-full rounded-lg border px-3 text-xs outline-none focus:border-[#346aea]"
                      style="border-color: rgb(0 0 0 / 0.15); color: #1a1a1a; background: rgb(0 0 0 / 0.02)"
                    />
                  </label>
                  <label class="block text-xs" style="color: #616161">
                    并发数（1~10）
                    <input
                      v-model.number="p.maxConcurrency"
                      type="number"
                      min="1"
                      max="10"
                      class="mt-1 h-9 w-full rounded-lg border px-3 text-xs outline-none focus:border-[#346aea]"
                      style="border-color: rgb(0 0 0 / 0.15); color: #1a1a1a; background: rgb(0 0 0 / 0.02)"
                    />
                  </label>
                  <label class="col-span-2 block text-xs" style="color: #616161">
                    API 地址（baseUrl，尾部 /v1 可省略）
                    <input
                      v-model="p.baseUrl"
                      placeholder="https://api.example.com"
                      class="mt-1 h-9 w-full rounded-lg border px-3 text-xs outline-none focus:border-[#346aea]"
                      style="border-color: rgb(0 0 0 / 0.15); color: #1a1a1a; background: rgb(0 0 0 / 0.02)"
                    />
                  </label>
                  <label class="col-span-2 block text-xs" style="color: #616161">
                    API Key
                    <input
                      v-model="p.apiKey"
                      type="password"
                      placeholder="sk-..."
                      autocomplete="off"
                      class="mt-1 h-9 w-full rounded-lg border px-3 text-xs outline-none focus:border-[#346aea]"
                      style="border-color: rgb(0 0 0 / 0.15); color: #1a1a1a; background: rgb(0 0 0 / 0.02)"
                    />
                  </label>
                  <label class="col-span-2 flex items-center gap-2 text-xs" style="color: #616161">
                    <input v-model="p.supportsResponseFormat" type="checkbox" />
                    供应商支持 response_format=b64_json（部分供应商如 YunWu 不支持，取消勾选）
                  </label>
                </div>
              </div>
            </div>
            <button
              class="mb-6 flex h-9 items-center gap-1.5 rounded-lg px-3 text-xs transition-colors hover:opacity-90"
              style="background: rgb(52 106 234 / 0.08); color: #346aea"
              @click="addProvider"
            >
              + 添加供应商
            </button>

            <!-- 下载设置 -->
            <h3 class="mb-3 text-sm font-medium" style="color: #1a1a1a">下载保存</h3>
            <div class="mb-2 rounded-xl border p-4" style="border-color: rgb(0 0 0 / 0.12)">
              <div class="mb-4 flex items-center gap-4">
                <span class="text-xs" style="color: #616161">保存格式</span>
                <div class="relative flex overflow-hidden rounded-lg p-0.5" style="background: rgb(0 0 0 / 0.04); width: 10rem">
                  <div
                    class="absolute inset-y-0.5 rounded-md transition-all duration-300 ease-out"
                    :style="{
                      left: `calc(${editSettings.saveFormat === 'png' ? 0 : 1} * 100% / 2 + 2px)`,
                      width: 'calc(100% / 2 - 4px)',
                      background: '#1a1a1a',
                    }"
                  ></div>
                  <button
                    v-for="f in ['png', 'jpg']"
                    :key="f"
                    class="relative h-8 flex-1 rounded-md text-xs font-medium transition-colors duration-150"
                    :style="{ color: editSettings.saveFormat === f ? '#fff' : '#616161' }"
                    @click="editSettings.saveFormat = f as 'png' | 'jpg'"
                  >
                    {{ f.toUpperCase() }}
                  </button>
                </div>
              </div>
              <div v-if="editSettings.saveFormat === 'jpg'" class="mb-4">
                <div class="mb-1.5 flex items-center justify-between text-xs" style="color: #616161">
                  <span>JPG 压缩质量</span>
                  <span class="font-semibold" style="color: #1a1a1a">
                    {{ Math.round(editSettings.jpgQuality * 100) }}%
                  </span>
                </div>
                <input
                  v-model.number="editSettings.jpgQuality"
                  type="range"
                  min="0.5"
                  max="1"
                  step="0.05"
                  class="w-full accent-[#346aea]"
                />
                <p class="mt-1 text-[10px]" style="color: #3c3a3a">
                  质量越高文件越大，100% 为最高画质
                </p>
              </div>
              <label class="block text-xs" style="color: #616161">
                保存路径（Win/macOS/Linux 生效；Android 将默认保存到系统相册）
                <div class="mt-1 flex gap-2">
                  <input
                    v-model="editSettings.saveDir"
                    placeholder="未设置：首次保存时弹出目录选择"
                    class="h-9 flex-1 rounded-lg border px-3 text-xs outline-none focus:border-[#346aea]"
                    style="border-color: rgb(0 0 0 / 0.15); color: #1a1a1a; background: rgb(0 0 0 / 0.02)"
                  />
                  <button
                    class="h-9 shrink-0 rounded-lg px-3 text-xs transition-colors hover:opacity-90"
                    style="background: rgb(0 0 0 / 0.06); color: #1a1a1a"
                    @click="chooseSaveDir"
                  >
                    浏览…
                  </button>
                  <button
                    v-if="editSettings.saveDir"
                    class="h-9 shrink-0 rounded-lg px-3 text-xs transition-colors hover:opacity-90"
                    style="background: rgb(0 0 0 / 0.06); color: #616161"
                    title="清除，恢复首次保存时询问"
                    @click="editSettings.saveDir = ''"
                  >
                    清除
                  </button>
                </div>
              </label>
            </div>

            <!-- 日志 -->
            <h3 class="mb-3 mt-6 text-sm font-medium" style="color: #1a1a1a">运行日志（最近 {{ logs.length }} 条）</h3>
            <div
              class="custom-scrollbar mb-6 max-h-48 overflow-y-auto rounded-xl p-3"
              style="background: rgb(0 0 0 / 0.03)"
            >
              <p v-if="logs.length === 0" class="text-xs" style="color: #919191">暂无日志</p>
              <p
                v-for="(l, i) in logs"
                :key="i"
                class="text-[11px] leading-relaxed"
                :style="{ color: l.level === 'error' ? '#d3482b' : '#616161' }"
              >
                [{{ l.time }}] {{ l.text }}
              </p>
            </div>

            <!-- 底部操作 -->
            <div class="flex justify-end gap-2">
              <button
                class="rounded-lg px-4 py-2 text-sm transition-colors hover:bg-black/10"
                style="background: rgb(0 0 0 / 0.04); color: #616161"
                @click="showSettings = false"
              >
                取消
              </button>
              <button
                class="rounded-lg px-4 py-2 text-sm font-medium text-white transition-colors"
                style="background: #346aea"
                @click="applySettings"
              >
                保存
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- Toast -->
    <Teleport to="body">
      <Transition name="modal">
        <div
          v-if="toast"
          class="fixed left-1/2 top-14 z-[60] -translate-x-1/2 rounded-xl px-4 py-2.5 text-xs shadow-lg"
          :style="toast.ok ? 'background: #1a1a1a; color: #fff' : 'background: #d3482b; color: #fff'"
        >
          {{ toast.text }}
        </div>
      </Transition>
    </Teleport>
  </div>
</template>
