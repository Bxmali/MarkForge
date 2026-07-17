# MarkForge

本地批量图片 / 视频水印桌面工具（React + Tauri 2）。

## 下载安装

- macOS：见仓库 `release/` 目录中的 `.dmg`
- Windows：`.exe` 需在 Windows 上打包（见下方）

## 开发运行

```bash
npm install
npm run tauri dev
```

## 打包

```bash
# macOS DMG
npm run tauri build -- --bundles dmg

# Windows NSIS 安装包（在 Windows 机器上）
npm run tauri build -- --bundles nsis
```

产物：`src-tauri/target/release/bundle/`

## 功能

- 图片 / 视频批量水印
- AI 预设开跑（图/视频可分别选处理方式）
- 手动参数模式
- 输出默认 Downloads
- 视频处理需本机安装 ffmpeg（`brew install ffmpeg`）
