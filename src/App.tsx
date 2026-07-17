import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { AnimatePresence, motion } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  Bot,
  CheckCircle2,
  Film,
  FolderOpen,
  FolderPlus,
  ImagePlus,
  KeyRound,
  Loader2,
  MessageCircleHeart,
  Play,
  RefreshCw,
  SendHorizontal,
  Settings2,
  SlidersHorizontal,
  Sparkles,
  Trash2,
  WandSparkles,
  X,
  XCircle,
} from "lucide-react";
import "./App.css";
import logoUrl from "./assets/markforge-logo.png";

type AppMode = "ai" | "manual";

type Position =
  | "top-left"
  | "top-center"
  | "top-right"
  | "center-left"
  | "center"
  | "center-right"
  | "bottom-left"
  | "bottom-center"
  | "bottom-right";

type CropInsets = {
  left: number;
  top: number;
  right: number;
  bottom: number;
};

type WatermarkOptions = {
  text: string;
  position: Position;
  opacity: number;
  fontScale: number;
  textColor: string;
  strokeColor: string;
  marginRatio: number;
  fontPath?: string | null;
  style: "single" | "tiled" | "moving";
  crop: CropInsets | null;
};

type AiSettings = {
  baseUrl: string;
  apiKey: string;
  model: string;
};

type QueueItem = {
  id: string;
  path: string;
  name: string;
  kind: "image" | "video";
  status: "queued" | "running" | "ok" | "failed";
  error?: string;
  output?: string;
};

type JobResult = {
  input: string;
  output?: string | null;
  ok: boolean;
  error?: string | null;
};

type ChatMsg = {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
};

type AiChatResponse = {
  reply: string;
  optionsPatch?: Partial<WatermarkOptions> | null;
  actions?: string[];
};

const AI_WELCOME: ChatMsg = {
  id: "welcome",
  role: "assistant",
    content:
      "嗨～先在左边加好图片/视频，再点「选择预设并开跑」✨\n有图又有视频时，可以分别选处理方式；也可以继续自己打字下指令。",
  };

const POSITIONS: { value: Position; label: string }[] = [
  { value: "top-left", label: "左上" },
  { value: "top-center", label: "上中" },
  { value: "top-right", label: "右上" },
  { value: "center-left", label: "左中" },
  { value: "center", label: "正中" },
  { value: "center-right", label: "右中" },
  { value: "bottom-left", label: "左下" },
  { value: "bottom-center", label: "下中" },
  { value: "bottom-right", label: "右下" },
];

const IMAGE_EXT = /\.(jpe?g|png|webp|gif|bmp|tiff?)$/i;
const VIDEO_EXT = /\.(mp4|mov|m4v|avi|mkv|webm)$/i;
const MEDIA_EXT = /\.(jpe?g|png|webp|gif|bmp|tiff?|mp4|mov|m4v|avi|mkv|webm)$/i;
const AI_STORAGE_KEY = "markforge.ai.settings.v1";
const MODE_STORAGE_KEY = "markforge.mode.v1";
const DEFAULT_WM_TEXT = "red.aiplanet.me/";

type LayoutPreset = {
  id: string;
  title: string;
  desc: string;
  apply: Partial<WatermarkOptions>;
};

type TextStylePreset = {
  id: string;
  title: string;
  desc: string;
  apply: Partial<WatermarkOptions>;
};

const IMAGE_LAYOUT_PRESETS: LayoutPreset[] = [
  {
    id: "img-bottom",
    title: "底部居中",
    desc: "固定在画面最下方正中",
    apply: {
      style: "single",
      position: "bottom-center",
      opacity: 0.55,
      crop: null,
    },
  },
  {
    id: "img-corner",
    title: "右下角角标",
    desc: "小号品牌角标，不挡主体",
    apply: {
      style: "single",
      position: "bottom-right",
      opacity: 0.5,
      crop: null,
    },
  },
  {
    id: "img-top-left",
    title: "左上角角标",
    desc: "放在左上，适合封面图",
    apply: {
      style: "single",
      position: "top-left",
      opacity: 0.5,
      crop: null,
    },
  },
  {
    id: "img-crop-corner",
    title: "裁切 + 右下角",
    desc: "先裁掉上下各 10%，再打角标",
    apply: {
      style: "single",
      position: "bottom-right",
      opacity: 0.55,
      crop: { left: 0, top: 0.1, right: 0, bottom: 0.1 },
    },
  },
  {
    id: "img-tiled",
    title: "满屏平铺",
    desc: "密铺防盗，适合单图传播",
    apply: { style: "tiled", opacity: 0.35, crop: null },
  },
];

const VIDEO_LAYOUT_PRESETS: LayoutPreset[] = [
  {
    id: "vid-moving",
    title: "动态漂浮",
    desc: "单个水印在画面里来回跑",
    apply: { style: "moving", opacity: 0.45, crop: null },
  },
  {
    id: "vid-bottom",
    title: "底部居中",
    desc: "固定在画面最下方正中",
    apply: {
      style: "single",
      position: "bottom-center",
      opacity: 0.55,
      crop: null,
    },
  },
  {
    id: "vid-corner",
    title: "右下角角标",
    desc: "小号品牌角标，不挡主体",
    apply: {
      style: "single",
      position: "bottom-right",
      opacity: 0.5,
      crop: null,
    },
  },
  {
    id: "vid-crop-corner",
    title: "裁切 + 右下角",
    desc: "先裁掉上下各 10%，再打角标",
    apply: {
      style: "single",
      position: "bottom-right",
      opacity: 0.55,
      crop: { left: 0, top: 0.1, right: 0, bottom: 0.1 },
    },
  },
  {
    id: "vid-tiled",
    title: "满屏平铺",
    desc: "整段视频密铺防盗水印",
    apply: { style: "tiled", opacity: 0.32, crop: null },
  },
];

type FontSizeId = "S" | "M" | "L";

const FONT_SIZE_PRESETS: { id: FontSizeId; label: string; scale: number }[] = [
  { id: "S", label: "小", scale: 0.024 },
  { id: "M", label: "中", scale: 0.034 },
  { id: "L", label: "大", scale: 0.048 },
];

const TEXT_STYLE_PRESETS: TextStylePreset[] = [
  {
    id: "white-black",
    title: "白字黑描边",
    desc: "经典清晰，通用场景",
    apply: { textColor: "#FFFFFF", strokeColor: "#000000" },
  },
  {
    id: "pink-cute",
    title: "粉色可爱",
    desc: "粉字白描边，更甜一点",
    apply: { textColor: "#FF6B9D", strokeColor: "#FFFFFF" },
  },
  {
    id: "soft-white",
    title: "淡白轻透",
    desc: "半透明白字，不抢主体",
    apply: { textColor: "#FFFFFF", strokeColor: "#FFFFFF", opacity: 0.32 },
  },
  {
    id: "black-white",
    title: "黑字白描边",
    desc: "亮色背景更清楚",
    apply: { textColor: "#1A1A1A", strokeColor: "#FFFFFF", opacity: 0.7 },
  },
  {
    id: "gold",
    title: "金色质感",
    desc: "金字深描边，偏高级",
    apply: { textColor: "#FFE08A", strokeColor: "#5A3A00", opacity: 0.7 },
  },
];

function mergePresetOptions(
  base: WatermarkOptions,
  layout: LayoutPreset,
  textStyle: TextStylePreset,
  fontSize: (typeof FONT_SIZE_PRESETS)[number],
  text: string,
): WatermarkOptions {
  const merged: WatermarkOptions = {
    ...base,
    ...layout.apply,
    ...textStyle.apply,
    text,
    fontScale: fontSize.scale,
  };
  if (textStyle.apply.opacity === undefined && layout.apply.opacity !== undefined) {
    merged.opacity = layout.apply.opacity;
  }
  return merged;
}

const MANUAL_HINT =
  "手动模式：调几个简单参数就能批量加水印。需要裁切 / 动态水印时，切到 AI 模式用预设一键搞定。";

function loadMode(): AppMode {
  try {
    const v = localStorage.getItem(MODE_STORAGE_KEY);
    if (v === "manual" || v === "ai") return v;
  } catch {
    /* ignore */
  }
  return "ai";
}

function cropActive(c: CropInsets | null) {
  if (!c) return false;
  return c.left > 0 || c.top > 0 || c.right > 0 || c.bottom > 0;
}

function toOptionsPayload(options: WatermarkOptions) {
  return {
    text: options.text,
    position: options.position,
    opacity: options.opacity,
    fontScale: options.fontScale,
    textColor: options.textColor,
    strokeColor: options.strokeColor,
    marginRatio: options.marginRatio,
    fontPath: options.fontPath || null,
    style: options.style,
    crop: cropActive(options.crop) ? options.crop : null,
  };
}

function parseOptionsPatch(
  patch: Partial<WatermarkOptions> & { crop?: CropInsets | null; style?: string },
): Partial<WatermarkOptions> {
  const next: Partial<WatermarkOptions> = {};
  if (typeof patch.text === "string") next.text = patch.text;
  if (typeof patch.position === "string") next.position = patch.position as Position;
  if (typeof patch.opacity === "number") next.opacity = patch.opacity;
  if (typeof patch.fontScale === "number") next.fontScale = patch.fontScale;
  if (typeof patch.textColor === "string") next.textColor = patch.textColor;
  if (typeof patch.strokeColor === "string") next.strokeColor = patch.strokeColor;
  if (typeof patch.marginRatio === "number") next.marginRatio = patch.marginRatio;
  if (typeof patch.style === "string") {
    const s = patch.style.toLowerCase();
    if (s === "tiled" || s === "diagonal") next.style = "tiled";
    else if (s === "moving" || s === "bounce" || s === "dynamic") next.style = "moving";
    else next.style = "single";
  }
  if (patch.crop === null) {
    next.crop = null;
  } else if (patch.crop && typeof patch.crop === "object") {
    const c = patch.crop;
    next.crop = {
      left: Number(c.left) || 0,
      top: Number(c.top) || 0,
      right: Number(c.right) || 0,
      bottom: Number(c.bottom) || 0,
    };
  }
  return next;
}

function basename(p: string) {
  const parts = p.split(/[/\\]/);
  return parts[parts.length - 1] || p;
}

function mediaKind(path: string): "image" | "video" | null {
  if (IMAGE_EXT.test(path)) return "image";
  if (VIDEO_EXT.test(path)) return "video";
  return null;
}

function appendQueueItems(prev: QueueItem[], paths: string[]): QueueItem[] {
  const exist = new Set(prev.map((x) => x.path));
  const next = [...prev];
  for (const path of paths) {
    const kind = mediaKind(path);
    if (!kind || exist.has(path)) continue;
    exist.add(path);
    next.push({
      id: `${path}-${uid()}`,
      path,
      name: basename(path),
      kind,
      status: "queued",
    });
  }
  return next;
}

function uid() {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function loadAiSettings(): AiSettings {
  try {
    const raw = localStorage.getItem(AI_STORAGE_KEY);
    if (raw) return { ...defaultAi(), ...JSON.parse(raw) };
  } catch {
    /* ignore */
  }
  return defaultAi();
}

function defaultAi(): AiSettings {
  return {
    baseUrl: "https://api.openai.com/v1",
    apiKey: "",
    model: "gpt-4o-mini",
  };
}

type BtnProps = {
  variant?: "primary" | "soft" | "ghost" | "danger";
  icon?: ReactNode;
  loading?: boolean;
  children?: ReactNode;
  className?: string;
  disabled?: boolean;
  onClick?: () => void;
};

function PinkButton({
  variant = "soft",
  icon,
  loading,
  children,
  className = "",
  disabled,
  onClick,
}: BtnProps) {
  return (
    <motion.button
      type="button"
      whileHover={disabled || loading ? undefined : { y: -1, scale: 1.02 }}
      whileTap={disabled || loading ? undefined : { scale: 0.97 }}
      className={`pink-btn ${variant} ${className}`}
      disabled={disabled || loading}
      onClick={onClick}
    >
      {loading ? <Loader2 size={16} className="spin" /> : icon}
      <span>{children}</span>
    </motion.button>
  );
}

function CuteBlob() {
  return (
    <svg className="decor-blob" viewBox="0 0 200 200" aria-hidden>
      <defs>
        <linearGradient id="g1" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#ffc2d4" />
          <stop offset="100%" stopColor="#ff8fab" />
        </linearGradient>
      </defs>
      <path
        fill="url(#g1)"
        d="M44.7,-67.2C57.3,-59.1,66.2,-45.3,73.4,-30.3C80.6,-15.3,86.1,0.9,82.8,15.8C79.5,30.7,67.4,44.3,53.3,55.4C39.2,66.5,23,75.1,5.2,78.1C-12.6,81.1,-31.9,78.5,-46.8,69.1C-61.7,59.7,-72.2,43.5,-78.1,25.6C-84,7.7,-85.3,-11.9,-78.6,-27.9C-71.9,-43.9,-57.2,-56.3,-41.6,-63.8C-26,-71.3,-9.5,-73.9,4.7,-80.4C18.9,-86.9,32.1,-75.3,44.7,-67.2Z"
        transform="translate(100 100)"
      />
    </svg>
  );
}

export default function App() {
  const [mode, setMode] = useState<AppMode>(() => loadMode());
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [outputDir, setOutputDir] = useState("");
  const [busy, setBusy] = useState(false);
  const [chatBusy, setChatBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [fonts, setFonts] = useState<string[]>([]);
  const [showAiSettings, setShowAiSettings] = useState(false);
  const [ai, setAi] = useState<AiSettings>(() => loadAiSettings());
  const [chatInput, setChatInput] = useState("");
  const [messages, setMessages] = useState<ChatMsg[]>([{ ...AI_WELCOME, id: "welcome" }]);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showPresetModal, setShowPresetModal] = useState(false);
  const [imageLayoutPreset, setImageLayoutPreset] = useState<LayoutPreset>(IMAGE_LAYOUT_PRESETS[0]);
  const [videoLayoutPreset, setVideoLayoutPreset] = useState<LayoutPreset>(VIDEO_LAYOUT_PRESETS[0]);
  const [textStylePreset, setTextStylePreset] = useState<TextStylePreset>(TEXT_STYLE_PRESETS[0]);
  const [fontSizeId, setFontSizeId] = useState<FontSizeId>("M");
  const [presetText, setPresetText] = useState(DEFAULT_WM_TEXT);
  const [dragOver, setDragOver] = useState(false);
  const chatStreamRef = useRef<HTMLDivElement>(null);
  const [options, setOptions] = useState<WatermarkOptions>({
    text: "red.aiplanet.me/",
    position: "bottom-right",
    opacity: 0.55,
    fontScale: 0.035,
    textColor: "#FFFFFF",
    strokeColor: "#000000",
    marginRatio: 0.04,
    fontPath: null,
    style: "single",
    crop: null,
  });

  useEffect(() => {
    void (async () => {
      try {
        const [text, fontList] = await Promise.all([
          invoke<string>("default_watermark_text"),
          invoke<string[]>("list_system_fonts"),
        ]);
        setOptions((o) => ({ ...o, text, fontPath: fontList[0] || null }));
        setFonts(fontList);
        setPresetText(text || DEFAULT_WM_TEXT);
      } catch {
        /* browser preview */
      }
      // Always fill Downloads: Rust dirs::download_dir first, then Tauri path API
      try {
        const fallback = await invoke<string>("default_output_dir");
        if (fallback) setOutputDir(fallback);
      } catch {
        try {
          const { downloadDir } = await import("@tauri-apps/api/path");
          const dir = await downloadDir();
          if (dir) setOutputDir(dir);
        } catch {
          /* browser preview */
        }
      }
    })();
  }, []);

  useEffect(() => {
    const locked = showPresetModal || showAiSettings;
    if (!locked) return;
    document.documentElement.classList.add("modal-lock");
    document.body.classList.add("modal-lock");
    const blockScroll = (e: WheelEvent | TouchEvent) => {
      const t = e.target as HTMLElement | null;
      if (t?.closest?.(".modal, .preset-select-list")) return;
      e.preventDefault();
    };
    window.addEventListener("wheel", blockScroll, { passive: false });
    window.addEventListener("touchmove", blockScroll, { passive: false });
    return () => {
      document.documentElement.classList.remove("modal-lock");
      document.body.classList.remove("modal-lock");
      window.removeEventListener("wheel", blockScroll);
      window.removeEventListener("touchmove", blockScroll);
    };
  }, [showPresetModal, showAiSettings]);

  useEffect(() => {
    localStorage.setItem(AI_STORAGE_KEY, JSON.stringify(ai));
  }, [ai]);

  useEffect(() => {
    localStorage.setItem(MODE_STORAGE_KEY, mode);
  }, [mode]);

  useEffect(() => {
    // 只滚聊天列表本身，避免 scrollIntoView 把整页拖到底部
    const el = chatStreamRef.current;
    if (!el) return;
    el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
  }, [messages, chatBusy]);

  const stats = useMemo(() => {
    const total = queue.length;
    const ok = queue.filter((q) => q.status === "ok").length;
    const failed = queue.filter((q) => q.status === "failed").length;
    const pending = queue.filter((q) => q.status === "queued" || q.status === "running").length;
    const images = queue.filter((q) => q.kind === "image").length;
    const videos = queue.filter((q) => q.kind === "video").length;
    return { total, ok, failed, pending, images, videos };
  }, [queue]);

  const fontSizePreset =
    FONT_SIZE_PRESETS.find((f) => f.id === fontSizeId) ?? FONT_SIZE_PRESETS[1];

  function patchOptions(partial: Partial<WatermarkOptions>) {
    setOptions((prev) => ({ ...prev, ...partial }));
  }

  function switchMode(next: AppMode) {
    if (next === mode) return;
    setMode(next);
    setError(null);
    setShowAdvanced(false);
  }

  function clearChat() {
    setMessages([{ ...AI_WELCOME, id: uid() }]);
    setShowAdvanced(false);
    setChatInput("");
  }

  function openPresetModal() {
    if (chatBusy || busy) return;
    if (!queue.length) {
      setError("请先添加图片或视频，再选择预设");
      return;
    }
    setPresetText(options.text.trim() || DEFAULT_WM_TEXT);
    setImageLayoutPreset(IMAGE_LAYOUT_PRESETS[0]);
    setVideoLayoutPreset(VIDEO_LAYOUT_PRESETS[0]);
    setTextStylePreset(TEXT_STYLE_PRESETS[0]);
    setFontSizeId("M");
    setShowPresetModal(true);
    setError(null);
  }

  function confirmPreset() {
    if (!queue.length) {
      setError("请先添加图片或视频");
      return;
    }
    const text = presetText.trim() || DEFAULT_WM_TEXT;
    const hasImages = queue.some((q) => q.kind === "image");
    const hasVideos = queue.some((q) => q.kind === "video");
    const imageOpts = hasImages
      ? mergePresetOptions(options, imageLayoutPreset, textStylePreset, fontSizePreset, text)
      : null;
    const videoOpts = hasVideos
      ? mergePresetOptions(options, videoLayoutPreset, textStylePreset, fontSizePreset, text)
      : null;
    const displayOpts = imageOpts ?? videoOpts ?? options;
    setOptions(displayOpts);
    setShowPresetModal(false);

    const parts: string[] = [];
    if (hasImages) parts.push(`图片「${imageLayoutPreset.title}」`);
    if (hasVideos) parts.push(`视频「${videoLayoutPreset.title}」`);
    parts.push(`文字「${textStylePreset.title}」`);
    parts.push(`字号${fontSizePreset.label}`);

    setMessages((m) => [
      ...m,
      {
        id: uid(),
        role: "user",
        content: `预设：${parts.join(" · ")}，文案「${text}」`,
      },
      {
        id: uid(),
        role: "assistant",
        content: `好的～按 ${parts.join(" / ")} 开跑，输出到：${outputDir || "请先选择目录"}`,
      },
    ]);
    void runBatch({ image: imageOpts ?? undefined, video: videoOpts ?? undefined });
  }

  const optionsPayload = useMemo(() => toOptionsPayload(options), [options]);

  const addPaths = useCallback((paths: string[]) => {
    const media = paths.filter((p) => MEDIA_EXT.test(p));
    if (!media.length) return;
    setQueue((prev) => appendQueueItems(prev, media));
  }, []);

  const pickMedia = useCallback(async () => {
    setError(null);
    const selected = await open({
      multiple: true,
      directory: false,
      filters: [
        {
          name: "图片与视频",
          extensions: [
            "jpg",
            "jpeg",
            "png",
            "webp",
            "gif",
            "bmp",
            "tif",
            "tiff",
            "mp4",
            "mov",
            "m4v",
            "avi",
            "mkv",
            "webm",
          ],
        },
      ],
    });
    if (!selected) return;
    addPaths(Array.isArray(selected) ? selected : [selected]);
  }, [addPaths]);

  const pickFolder = useCallback(async () => {
    setError(null);
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== "string" || !dir) return;
    try {
      const files = await invoke<string[]>("list_media_files", { dir });
      if (!files.length) {
        setError("该文件夹里没有可识别的图片/视频");
        return;
      }
      addPaths(files);
      setInfo(`已从文件夹加载 ${files.length} 个素材`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [addPaths]);

  const pickImages = pickMedia;

  const pickOutputDir = useCallback(async () => {
    setError(null);
    const dir = await open({
      directory: true,
      multiple: false,
      defaultPath: outputDir || undefined,
    });
    if (typeof dir === "string" && dir) setOutputDir(dir);
  }, [outputDir]);

  const openOutput = useCallback(async () => {
    if (!outputDir) return;
    try {
      await openPath(outputDir);
    } catch {
      try {
        await revealItemInDir(outputDir);
      } catch (e) {
        setError(`无法打开输出目录：${e instanceof Error ? e.message : String(e)}`);
      }
    }
  }, [outputDir]);

  const removeItem = useCallback(
    (id: string) => {
      if (busy) return;
      setQueue((q) => q.filter((x) => x.id !== id));
    },
    [busy],
  );

  const clearDone = () => {
    setQueue((q) => q.filter((x) => x.status === "queued" || x.status === "running"));
  };

  const clearAll = () => {
    if (busy) return;
    setQueue([]);
    setInfo(null);
  };

  const retryFailed = () => {
    setQueue((q) =>
      q.map((item) =>
        item.status === "failed"
          ? { ...item, status: "queued", error: undefined, output: undefined }
          : item,
      ),
    );
  };

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void (async () => {
      try {
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        unlisten = await getCurrentWebview().onDragDropEvent((event) => {
          if (event.payload.type === "over" || event.payload.type === "enter") {
            setDragOver(true);
          } else if (event.payload.type === "leave") {
            setDragOver(false);
          } else if (event.payload.type === "drop") {
            setDragOver(false);
            addPaths(event.payload.paths);
          }
        });
      } catch {
        /* browser preview */
      }
    })();
    return () => {
      unlisten?.();
    };
  }, [addPaths]);

  const runBatch = useCallback(
    async (
      override?:
        | WatermarkOptions
        | { image?: WatermarkOptions; video?: WatermarkOptions },
    ) => {
      if (!queue.length) {
        setError("请先添加图片或视频");
        return;
      }
      if (!outputDir) {
        setError("请选择输出目录");
        return;
      }
      const pending = queue.filter((q) => q.status === "queued" || q.status === "failed");
      if (!pending.length) {
        setError("没有待处理任务，可先「重试失败」");
        return;
      }

      const split =
        override && typeof override === "object" && ("image" in override || "video" in override)
          ? (override as { image?: WatermarkOptions; video?: WatermarkOptions })
          : null;
      const single = split ? null : ((override as WatermarkOptions | undefined) ?? options);

      setBusy(true);
      setError(null);
      setInfo(null);
      const ids = new Set(pending.map((p) => p.id));
      setQueue((q) =>
        q.map((item) => (ids.has(item.id) ? { ...item, status: "running", error: undefined } : item)),
      );

      const applyGroup = async (items: QueueItem[], opts: WatermarkOptions) => {
        if (!items.length) return [] as JobResult[];
        return invoke<JobResult[]>("apply_watermark_batch", {
          inputs: items.map((p) => p.path),
          outputDir,
          options: toOptionsPayload(opts),
        });
      };

      try {
        let results: JobResult[] = [];
        if (split) {
          const imgs = pending.filter((p) => p.kind === "image");
          const vids = pending.filter((p) => p.kind === "video");
          const imgOpts = split.image ?? options;
          const vidOpts = split.video ?? options;
          const [imgRes, vidRes] = await Promise.all([
            applyGroup(imgs, imgOpts),
            applyGroup(vids, vidOpts),
          ]);
          results = [...imgRes, ...vidRes];
        } else {
          results = await applyGroup(pending, single!);
        }

        const byInput = new Map(results.map((r) => [r.input, r]));
        setQueue((q) =>
          q.map((item) => {
            if (!ids.has(item.id)) return item;
            const r = byInput.get(item.path);
            if (!r) return { ...item, status: "failed", error: "无返回结果" };
            return r.ok
              ? { ...item, status: "ok", output: r.output || undefined, error: undefined }
              : { ...item, status: "failed", error: r.error || "处理失败", output: undefined };
          }),
        );
        const okN = results.filter((r) => r.ok).length;
        setInfo(`完成啦～成功 ${okN}，失败 ${results.length - okN}`);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        setQueue((q) =>
          q.map((item) =>
            ids.has(item.id) && item.status === "running"
              ? { ...item, status: "failed", error: "批次中断" }
              : item,
          ),
        );
      } finally {
        setBusy(false);
      }
    },
    [options, outputDir, queue],
  );

  const applyAiActions = useCallback(
    async (actions: string[], optsForBatch?: WatermarkOptions) => {
      for (const a of actions) {
        if (a === "pick_images") await pickImages();
        if (a === "pick_output") await pickOutputDir();
        if (a === "open_output") await openOutput();
        if (a === "clear_queue") clearAll();
        if (a === "retry_failed") retryFailed();
        if (a === "start_batch") await runBatch(optsForBatch);
      }
    },
    [openOutput, pickImages, pickOutputDir, runBatch],
  );

  async function sendChat(text?: string) {
    const content = (text ?? chatInput).trim();
    if (!content || chatBusy) return;
    if (!queue.length) {
      setError("请先添加图片或视频，再发送指令");
      return;
    }
    setChatInput("");
    setError(null);
    const userMsg: ChatMsg = { id: uid(), role: "user", content };
    setMessages((m) => [...m, userMsg]);
    setChatBusy(true);
    try {
      const history = messages
        .filter((m) => m.role === "user" || m.role === "assistant")
        .slice(-10)
        .map((m) => ({ role: m.role, content: m.content }));
      const res = await invoke<AiChatResponse>("ai_chat", {
        request: {
          settings: ai,
          userMessage: content,
          history,
          currentOptions: optionsPayload,
          queueCount: queue.length,
          outputDirSet: Boolean(outputDir),
        },
      });
      let merged = options;
      if (res.optionsPatch) {
        const next = parseOptionsPatch(
          res.optionsPatch as Partial<WatermarkOptions> & {
            crop?: CropInsets | null;
            style?: string;
          },
        );
        merged = { ...options, ...next };
        patchOptions(next);
      }
      setMessages((m) => [
        ...m,
        { id: uid(), role: "assistant", content: res.reply || "好的～" },
      ]);
      if (res.actions?.length) await applyAiActions(res.actions, merged);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setMessages((m) => [
        ...m,
        {
          id: uid(),
          role: "assistant",
          content: `呜，连不上 AI：${msg}\n先打开右上角「AI 设置」检查中转站、密钥和模型哦。`,
        },
      ]);
    } finally {
      setChatBusy(false);
    }
  }

  return (
    <div className={`shell mode-${mode}`}>
      <CuteBlob />
      <header className="hero">
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.45 }}
          className="hero-copy"
        >
          <div className="brand-row">
            <img
              className="brand-logo"
              src={logoUrl}
              alt="MarkForge"
              width={40}
              height={40}
              draggable={false}
            />
            <p className="brand">MarkForge</p>
          </div>
          <h1>{mode === "ai" ? "AI 水印导演" : "手动批量水印"}</h1>
          <p className="sub">
            {mode === "ai"
              ? "一句话完成裁切 · 动态水印 · 批量导出，无需拧参数"
              : "简单调几个参数，批量本地加水印"}
          </p>
        </motion.div>
        <div className="hero-actions">
          <div className="mode-switch" role="tablist" aria-label="工作模式">
            <button
              type="button"
              role="tab"
              aria-selected={mode === "ai"}
              className={mode === "ai" ? "active" : ""}
              onClick={() => switchMode("ai")}
            >
              <WandSparkles size={14} />
              AI 模式
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={mode === "manual"}
              className={mode === "manual" ? "active" : ""}
              onClick={() => switchMode("manual")}
            >
              <SlidersHorizontal size={14} />
              手动模式
            </button>
          </div>
          {mode === "ai" ? (
            <PinkButton
              variant="ghost"
              icon={<Settings2 size={16} />}
              onClick={() => setShowAiSettings(true)}
            >
              AI 设置
            </PinkButton>
          ) : null}
          <PinkButton
            variant="primary"
            icon={<Play size={16} />}
            loading={busy}
            onClick={() => void runBatch()}
          >
            开始批量
          </PinkButton>
        </div>
      </header>

      <main className={`grid layout-${mode}`}>
        <motion.section
          className="panel media"
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.05 }}
        >
          <div className="panel-head">
            <h2>
              <ImagePlus size={16} /> 素材队列
            </h2>
            <div className="row">
              <PinkButton icon={<ImagePlus size={15} />} onClick={() => void pickMedia()} disabled={busy}>
                添加素材
              </PinkButton>
              <PinkButton icon={<FolderPlus size={15} />} onClick={() => void pickFolder()} disabled={busy}>
                添加文件夹
              </PinkButton>
              <PinkButton icon={<RefreshCw size={15} />} variant="ghost" onClick={retryFailed} disabled={busy}>
                重试失败
              </PinkButton>
              <PinkButton icon={<Trash2 size={15} />} variant="ghost" onClick={clearDone} disabled={busy}>
                清完成
              </PinkButton>
              <PinkButton icon={<Trash2 size={15} />} variant="danger" onClick={clearAll} disabled={busy}>
                清空
              </PinkButton>
            </div>
          </div>
          <p className="meta">
            共 {stats.total} · 成功 {stats.ok} · 失败 {stats.failed} · 待处理 {stats.pending}
            {mode === "ai" ? (
              <>
                {" "}
                ·{" "}
                {options.style === "moving"
                  ? "动态来回跑"
                  : options.style === "tiled"
                    ? "满屏平铺"
                    : "单点水印"}
                {cropActive(options.crop) ? " · 已裁切" : ""}
              </>
            ) : null}
          </p>
          <div
            className={`list ${dragOver ? "drag-over" : ""}`}
            onDragOver={(e) => {
              e.preventDefault();
              setDragOver(true);
            }}
            onDragLeave={() => setDragOver(false)}
            onDrop={(e) => {
              e.preventDefault();
              setDragOver(false);
            }}
          >
            {queue.length === 0 ? (
              <motion.button
                type="button"
                className="drop"
                whileHover={{ scale: 1.01 }}
                whileTap={{ scale: 0.99 }}
                onClick={() => void pickMedia()}
              >
                <Sparkles size={28} color="#ff6b9d" />
                <strong>拖入图片/视频，或点这里选择</strong>
                <span>支持多选 · 也可「添加文件夹」批量加载</span>
              </motion.button>
            ) : (
              <AnimatePresence initial={false}>
                {queue.map((item) => (
                  <motion.div
                    key={item.id}
                    layout
                    initial={{ opacity: 0, y: 8 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, scale: 0.96 }}
                    className={`item status-${item.status}`}
                  >
                    <div className="item-main">
                      <span className="kind-ico" title={item.kind === "video" ? "视频" : "图片"}>
                        {item.kind === "video" ? <Film size={14} /> : <ImagePlus size={14} />}
                      </span>
                      <span className="name" title={item.path}>
                        {item.name}
                      </span>
                      <span className="badge">
                        {item.status === "ok" ? <CheckCircle2 size={12} /> : null}
                        {item.status === "failed" ? <XCircle size={12} /> : null}
                        {statusLabel(item.status)}
                      </span>
                      <button
                        type="button"
                        className="item-remove"
                        title="移除"
                        disabled={busy || item.status === "running"}
                        onClick={() => removeItem(item.id)}
                      >
                        <X size={14} />
                      </button>
                    </div>
                    {item.error ? <p className="err-line">{item.error}</p> : null}
                    {item.output ? <p className="ok-line">{item.output}</p> : null}
                  </motion.div>
                ))}
              </AnimatePresence>
            )}
          </div>
        </motion.section>

        <div className="side">
          {mode === "ai" ? (
            <motion.section
              className="panel chat chat-ai"
              key="chat"
              initial={{ opacity: 0, y: 16 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.08 }}
            >
              <div className="panel-head">
                <h2>
                  <MessageCircleHeart size={16} /> AI 对话 · 一句话搞定
                </h2>
                <div className="advanced-wrap">
                  <button
                    type="button"
                    className={`pill advanced-btn ${showAdvanced ? "open" : ""}`}
                    onClick={() => setShowAdvanced((v) => !v)}
                  >
                    高级
                  </button>
                  {showAdvanced ? (
                    <div className="advanced-menu">
                      <button type="button" onClick={clearChat} disabled={chatBusy}>
                        <Trash2 size={14} />
                        清除聊天记录
                      </button>
                    </div>
                  ) : null}
                </div>
              </div>
              <div className="chat-stream" ref={chatStreamRef}>
                <AnimatePresence initial={false}>
                  {messages.map((m) => (
                    <motion.div
                      key={m.id}
                      initial={{ opacity: 0, y: 8 }}
                      animate={{ opacity: 1, y: 0 }}
                      className={`bubble ${m.role}`}
                    >
                      {m.role === "assistant" ? <Bot size={14} /> : null}
                      <p>{m.content}</p>
                    </motion.div>
                  ))}
                </AnimatePresence>
                {chatBusy ? (
                  <div className="bubble assistant typing">
                    <Loader2 size={14} className="spin" />
                    <p>粉粉在编排…</p>
                  </div>
                ) : null}
              </div>

              <div className="chat-composer">
                <PinkButton
                  variant="soft"
                  className="preset-trigger"
                  icon={<WandSparkles size={15} />}
                  disabled={chatBusy || busy || !queue.length}
                  onClick={openPresetModal}
                >
                  选择预设并开跑
                </PinkButton>

                <div className="composer-stack">
                  <label className="composer-field">
                    <span className="composer-label">跟 AI 说</span>
                    <div className="composer-row">
                      <input
                        value={chatInput}
                        onChange={(e) => setChatInput(e.target.value)}
                        placeholder={
                          queue.length
                            ? "也可以继续自己输入需求…"
                            : "请先在左侧添加图片/视频"
                        }
                        disabled={!queue.length || chatBusy}
                        onKeyDown={(e) => {
                          if (e.key === "Enter" && !e.shiftKey) {
                            e.preventDefault();
                            void sendChat();
                          }
                        }}
                      />
                      <PinkButton
                        variant="primary"
                        className="composer-action"
                        icon={<SendHorizontal size={16} />}
                        loading={chatBusy}
                        disabled={!queue.length}
                        onClick={() => void sendChat()}
                      >
                        发送
                      </PinkButton>
                    </div>
                  </label>

                  <label className="composer-field">
                    <span className="composer-label">
                      输出目录
                      {outputDir ? (
                        <button type="button" className="open-dir-link" onClick={() => void openOutput()}>
                          打开
                        </button>
                      ) : null}
                    </span>
                    <div className="composer-row">
                      <input readOnly value={outputDir} placeholder="默认 Downloads" />
                      <PinkButton
                        className="composer-action"
                        icon={<FolderOpen size={15} />}
                        onClick={() => void pickOutputDir()}
                        disabled={busy}
                      >
                        选择
                      </PinkButton>
                    </div>
                  </label>
                </div>

                {error ? <div className="banner err">{error}</div> : null}
                {info ? <div className="banner ok">{info}</div> : null}
              </div>
            </motion.section>
          ) : (
            <motion.section
              className="panel settings settings-manual"
              key="manual"
              initial={{ opacity: 0, y: 16 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.08 }}
            >
              <div className="panel-head">
                <h2>
                  <SlidersHorizontal size={16} /> 简单参数
                </h2>
                <span className="pill soft">手动</span>
              </div>
              <p className="manual-hint">{MANUAL_HINT}</p>
              <label className="field">
                <span>水印文案</span>
                <input
                  value={options.text}
                  onChange={(e) => patchOptions({ text: e.target.value })}
                  placeholder="red.aiplanet.me/"
                />
              </label>
              <label className="field">
                <span>位置</span>
                <select
                  value={options.position}
                  onChange={(e) => patchOptions({ position: e.target.value as Position })}
                >
                  {POSITIONS.map((p) => (
                    <option key={p.value} value={p.value}>
                      {p.label}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span>透明度 {Math.round(options.opacity * 100)}%</span>
                <input
                  type="range"
                  min={0.15}
                  max={0.95}
                  step={0.05}
                  value={options.opacity}
                  onChange={(e) => patchOptions({ opacity: Number(e.target.value) })}
                />
              </label>
              <label className="field">
                <span>字号 {options.fontScale.toFixed(3)}</span>
                <input
                  type="range"
                  min={0.018}
                  max={0.08}
                  step={0.002}
                  value={options.fontScale}
                  onChange={(e) => patchOptions({ fontScale: Number(e.target.value) })}
                />
              </label>
              <div className="colors">
                <label className="field">
                  <span>文字色</span>
                  <input
                    type="color"
                    value={options.textColor}
                    onChange={(e) => patchOptions({ textColor: e.target.value })}
                  />
                </label>
                <label className="field">
                  <span>描边色</span>
                  <input
                    type="color"
                    value={options.strokeColor}
                    onChange={(e) => patchOptions({ strokeColor: e.target.value })}
                  />
                </label>
              </div>
              <label className="field">
                <span>字体</span>
                <select
                  value={options.fontPath || ""}
                  onChange={(e) => patchOptions({ fontPath: e.target.value || null })}
                >
                  <option value="">自动</option>
                  {fonts.map((f) => (
                    <option key={f} value={f}>
                      {basename(f)}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span>输出目录</span>
                <div className="row stretch">
                  <input readOnly value={outputDir} placeholder="选择导出文件夹" />
                  <PinkButton icon={<FolderOpen size={15} />} onClick={() => void pickOutputDir()} disabled={busy}>
                    选择
                  </PinkButton>
                </div>
              </label>
              {outputDir ? (
                <PinkButton
                  variant="ghost"
                  className="full"
                  icon={<FolderOpen size={15} />}
                  onClick={() => void openOutput()}
                >
                  打开输出目录
                </PinkButton>
              ) : null}
              {error ? <div className="banner err">{error}</div> : null}
              {info ? <div className="banner ok">{info}</div> : null}
            </motion.section>
          )}
        </div>
      </main>

      <AnimatePresence>
        {showPresetModal ? (
          <motion.div
            className="modal-root"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={() => setShowPresetModal(false)}
          >
            <motion.div
              className="modal preset-modal"
              initial={{ opacity: 0, y: 20, scale: 0.96 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: 12, scale: 0.98 }}
              onClick={(e) => e.stopPropagation()}
            >
              <div className="modal-head">
                <h3>
                  <WandSparkles size={18} /> 预设开跑
                </h3>
                <PinkButton variant="ghost" onClick={() => setShowPresetModal(false)}>
                  取消
                </PinkButton>
              </div>

              <p className="preset-queue-hint">
                队列里有 {stats.images} 张图、{stats.videos} 个视频
                {stats.images > 0 && stats.videos > 0 ? " · 可分别选择处理方式" : ""}
              </p>

              <div
                className={`preset-columns cols-${
                  stats.images > 0 && stats.videos > 0 ? 3 : 2
                }`}
              >
                {stats.images > 0 ? (
                  <div className="preset-col">
                    <p className="preset-section-title">图片处理方式</p>
                    <div className="preset-select-list" role="listbox" aria-label="图片处理方式">
                      {IMAGE_LAYOUT_PRESETS.map((p) => {
                        const active = imageLayoutPreset.id === p.id;
                        return (
                          <button
                            key={p.id}
                            type="button"
                            role="option"
                            aria-selected={active}
                            className={`preset-option ${active ? "active" : ""}`}
                            onClick={() => setImageLayoutPreset(p)}
                          >
                            <span className="preset-option-radio" />
                            <span className="preset-option-copy">
                              <strong>{p.title}</strong>
                              <span>{p.desc}</span>
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  </div>
                ) : null}

                {stats.videos > 0 ? (
                  <div className="preset-col">
                    <p className="preset-section-title">视频处理方式</p>
                    <div className="preset-select-list" role="listbox" aria-label="视频处理方式">
                      {VIDEO_LAYOUT_PRESETS.map((p) => {
                        const active = videoLayoutPreset.id === p.id;
                        return (
                          <button
                            key={p.id}
                            type="button"
                            role="option"
                            aria-selected={active}
                            className={`preset-option ${active ? "active" : ""}`}
                            onClick={() => setVideoLayoutPreset(p)}
                          >
                            <span className="preset-option-radio" />
                            <span className="preset-option-copy">
                              <strong>{p.title}</strong>
                              <span>{p.desc}</span>
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  </div>
                ) : null}

                <div className="preset-col">
                  <p className="preset-section-title">文字预设</p>
                  <div className="preset-select-list" role="listbox" aria-label="文字预设">
                    {TEXT_STYLE_PRESETS.map((p) => {
                      const active = textStylePreset.id === p.id;
                      return (
                        <button
                          key={p.id}
                          type="button"
                          role="option"
                          aria-selected={active}
                          className={`preset-option ${active ? "active" : ""}`}
                          onClick={() => setTextStylePreset(p)}
                        >
                          <span className="preset-option-radio" />
                          <span className="preset-option-copy">
                            <strong>{p.title}</strong>
                            <span>{p.desc}</span>
                          </span>
                        </button>
                      );
                    })}
                  </div>
                </div>
              </div>

              <div className="preset-footer-row">
                <div className="field preset-size-field">
                  <span>字体大小</span>
                  <div className="size-switch" role="group" aria-label="字体大小">
                    {FONT_SIZE_PRESETS.map((f) => (
                      <button
                        key={f.id}
                        type="button"
                        className={fontSizeId === f.id ? "active" : ""}
                        onClick={() => setFontSizeId(f.id)}
                      >
                        {f.label}
                      </button>
                    ))}
                  </div>
                </div>
                <label className="field preset-text-field">
                  <span>水印文案</span>
                  <input
                    autoFocus
                    value={presetText}
                    onChange={(e) => setPresetText(e.target.value)}
                    placeholder={DEFAULT_WM_TEXT}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        confirmPreset();
                      }
                    }}
                  />
                </label>
              </div>

              <PinkButton
                variant="primary"
                className="full"
                icon={<Play size={16} />}
                onClick={confirmPreset}
              >
                确认并开跑
              </PinkButton>
            </motion.div>
          </motion.div>
        ) : null}
      </AnimatePresence>

      <AnimatePresence>
        {showAiSettings ? (
          <motion.div
            className="modal-root"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={() => setShowAiSettings(false)}
          >
            <motion.div
              className="modal"
              initial={{ opacity: 0, y: 20, scale: 0.96 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: 12, scale: 0.98 }}
              onClick={(e) => e.stopPropagation()}
            >
              <div className="modal-head">
                <h3>
                  <KeyRound size={18} /> AI 中转站设置
                </h3>
                <PinkButton variant="ghost" onClick={() => setShowAiSettings(false)}>
                  关闭
                </PinkButton>
              </div>
              <p className="modal-hint">
                兼容 OpenAI 格式：Base URL 填到 <code>/v1</code>，例如{" "}
                <code>https://xxx.com/v1</code>
              </p>
              <label className="field">
                <span>中转站 Base URL</span>
                <input
                  value={ai.baseUrl}
                  onChange={(e) => setAi((s) => ({ ...s, baseUrl: e.target.value }))}
                  placeholder="https://api.openai.com/v1"
                />
              </label>
              <label className="field">
                <span>API Key（sk-…）</span>
                <input
                  type="password"
                  value={ai.apiKey}
                  onChange={(e) => setAi((s) => ({ ...s, apiKey: e.target.value }))}
                  placeholder="sk-..."
                />
              </label>
              <label className="field">
                <span>模型</span>
                <input
                  value={ai.model}
                  onChange={(e) => setAi((s) => ({ ...s, model: e.target.value }))}
                  placeholder="gpt-4o-mini"
                />
              </label>
              <PinkButton
                variant="primary"
                className="full"
                icon={<Sparkles size={16} />}
                onClick={() => {
                  setShowAiSettings(false);
                  setMessages((m) => [
                    ...m,
                    {
                      id: uid(),
                      role: "assistant",
                      content: "AI 设置已保存～可以继续跟我聊天调水印啦。",
                    },
                  ]);
                }}
              >
                保存并关闭
              </PinkButton>
            </motion.div>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

function statusLabel(s: QueueItem["status"]) {
  switch (s) {
    case "queued":
      return "排队";
    case "running":
      return "处理中";
    case "ok":
      return "完成";
    case "failed":
      return "失败";
  }
}
