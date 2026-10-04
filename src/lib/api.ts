// ai-image 业务层：设置持久化、尺寸计算、格式转换、保存封装
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Store } from "@tauri-apps/plugin-store";

export interface Provider {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  supportsResponseFormat: boolean;
  maxConcurrency: number;
  /** 可用模型列表（设置里拉取后存储） */
  models: string[];
  /** 当前选中的模型 */
  selectedModel: string;
}

export interface Settings {
  providers: Provider[];
  activeProviderId: string;
  /** 保存格式：png | jpg */
  saveFormat: "png" | "jpg";
  /** 保存目录（桌面端）；为空时首次保存会弹目录选择 */
  saveDir: string;
  /** jpg 压缩质量 0.5 ~ 1 */
  jpgQuality: number;
}

export const MODEL_NAME = "gpt-image-2";

/** 模型过滤：包含 image / banana / imagen 的模型优先保留 */
export function pickImageModels(ids: string[]): string[] {
  const matched = ids.filter((id) => /image|banana|imagen/i.test(id));
  return matched.length > 0 ? matched.sort() : [];
}

export const DEFAULT_SETTINGS: Settings = {
  providers: [
    {
      id: "nowcoding",
      name: "NowCoding",
      baseUrl: "https://nowcoding.ai",
      apiKey: "",
      supportsResponseFormat: true,
      maxConcurrency: 4,
      models: [],
      selectedModel: MODEL_NAME,
    },
    {
      id: "yunwu",
      name: "YunWu",
      baseUrl: "https://yunwu.ai",
      apiKey: "",
      supportsResponseFormat: false,
      maxConcurrency: 1,
      models: [],
      selectedModel: MODEL_NAME,
    },
  ],
  activeProviderId: "nowcoding",
  saveFormat: "png",
  saveDir: "",
  jpgQuality: 0.95,
};

let storePromise: Promise<Store> | null = null;
function getStore(): Promise<Store> {
  if (!storePromise) {
    storePromise = Store.load("settings.json");
  }
  return storePromise;
}

export async function loadSettings(): Promise<Settings> {
  try {
    const store = await getStore();
    const saved = await store.get<Partial<Settings>>("settings");
    if (!saved) return structuredClone(DEFAULT_SETTINGS);
    // 与默认值合并，保证新增字段有兜底（旧配置的供应商补 models/selectedModel）
    return {
      ...structuredClone(DEFAULT_SETTINGS),
      ...saved,
      providers:
        saved.providers && saved.providers.length > 0
          ? saved.providers.map((p) => ({
              ...p,
              models: p.models ?? [],
              selectedModel: p.selectedModel ?? MODEL_NAME,
            }))
          : structuredClone(DEFAULT_SETTINGS.providers),
    };
  } catch {
    return structuredClone(DEFAULT_SETTINGS);
  }
}

export async function saveSettings(s: Settings): Promise<void> {
  const store = await getStore();
  await store.set("settings", JSON.parse(JSON.stringify(s)));
  await store.save();
}

// ---------------- 尺寸计算（对齐原项目：16px 对齐 + 面积等比） ----------------

export const RATIOS = [
  { value: "auto", label: "自动" },
  { value: "1:1", label: "1:1" },
  { value: "4:3", label: "4:3" },
  { value: "3:4", label: "3:4" },
  { value: "16:9", label: "16:9" },
  { value: "9:16", label: "9:16" },
  { value: "3:2", label: "3:2" },
  { value: "2:3", label: "2:3" },
] as const;

export const TIERS = [
  { value: "1k", label: "1K", estimate: "~60s", base: 1024 },
  { value: "2k", label: "2K", estimate: "~90s", base: 2048 },
  { value: "4k", label: "4K", estimate: "~120s", base: 4096 },
] as const;

const align16 = (n: number) => Math.max(16, 16 * Math.round(n / 16));

/** 根据比例与像素档位计算 size 参数；auto 比例返回 "auto" */
export function computeSize(ratio: string, tier: string): string {
  if (ratio === "auto") return "auto";
  const [rw, rh] = ratio.split(":").map(Number);
  const t = TIERS.find((x) => x.value === tier) ?? TIERS[0];
  const w = align16(Math.sqrt((t.base * t.base * rw) / rh));
  const h = align16((w * rh) / rw);
  return `${w}x${h}`;
}

// ---------------- base64 / 格式转换 ----------------

export function b64ToBlobUrl(b64: string, mime: string): string {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return URL.createObjectURL(new Blob([bytes], { type: mime }));
}

/** png base64 → jpeg base64（Canvas 重编码） */
export function pngB64ToJpegB64(b64: string, quality = 0.95): Promise<string> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = img.naturalWidth;
      canvas.height = img.naturalHeight;
      const ctx = canvas.getContext("2d");
      if (!ctx) return reject(new Error("无法创建画布"));
      // JPEG 无透明通道，铺白底
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(img, 0, 0);
      const dataUrl = canvas.toDataURL("image/jpeg", quality);
      resolve(dataUrl.slice(dataUrl.indexOf(",") + 1));
    };
    img.onerror = () => reject(new Error("图片解码失败"));
    img.src = `data:image/png;base64,${b64}`;
  });
}

/** 读取图片宽高 */
export function imageSize(b64: string, mime: string): Promise<{ w: number; h: number }> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => resolve({ w: img.naturalWidth, h: img.naturalHeight });
    img.onerror = () => resolve({ w: 0, h: 0 });
    img.src = `data:${mime};base64,${b64}`;
  });
}

// ---------------- 生成与保存 ----------------

export interface GenResult {
  image_b64: string;
  mime: string;
  model: string;
}

/** 供应商连通测试结果 */
export interface TestResult {
  latency_ms: number;
  model_count: number;
}

/** 测试供应商连通性（GET /v1/models） */
export async function testProvider(baseUrl: string, apiKey: string): Promise<TestResult> {
  return invoke<TestResult>("test_provider", { baseUrl, apiKey });
}

/** 拉取供应商全部模型 ID 列表 */
export async function fetchModels(baseUrl: string, apiKey: string): Promise<string[]> {
  return invoke<string[]>("fetch_models", { baseUrl, apiKey });
}

export async function callGenerate(args: {
  baseUrl: string;
  apiKey: string;
  model: string;
  prompt: string;
  size: string;
  quality: string;
  responseFormat?: string | null;
}): Promise<GenResult> {
  return invoke<GenResult>("generate_image", {
    baseUrl: args.baseUrl,
    apiKey: args.apiKey,
    model: args.model,
    prompt: args.prompt,
    size: args.size,
    quality: args.quality,
    responseFormat: args.responseFormat ?? null,
  });
}

export async function callEdit(args: {
  baseUrl: string;
  apiKey: string;
  model: string;
  prompt: string;
  size: string;
  quality: string;
  imagesB64: string[];
  responseFormat?: string | null;
}): Promise<GenResult> {
  return invoke<GenResult>("edit_image", {
    baseUrl: args.baseUrl,
    apiKey: args.apiKey,
    model: args.model,
    prompt: args.prompt,
    size: args.size,
    quality: args.quality,
    imagesB64: args.imagesB64,
    responseFormat: args.responseFormat ?? null,
  });
}

/** 选择保存目录（桌面端系统目录选择框） */
export async function pickDirectory(): Promise<string | null> {
  const picked = await open({ directory: true, multiple: false, title: "选择保存目录" });
  return typeof picked === "string" ? picked : null;
}

/** 保存图片：jpg 时先经 Canvas 转换，再交给 Rust 落盘 */
export async function saveImageFile(args: {
  b64: string;
  mime: string;
  format: "png" | "jpg";
  jpgQuality: number;
  dir: string;
  filename: string;
}): Promise<string> {
  let dataB64 = args.b64;
  if (args.format === "jpg") {
    dataB64 = await pngB64ToJpegB64(args.b64, args.jpgQuality);
  }
  const path = await invoke<string>("save_image", {
    dataB64,
    dir: args.dir,
    filename: args.filename,
  });
  return path;
}

export function timestampName(prefix = "ai-image"): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return (
    `${prefix}_${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}` +
    `_${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}` +
    `_${Math.floor(Math.random() * 1000)}`
  );
}
