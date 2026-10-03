# ai-image 一键打包脚本：Inno Setup 安装包 + 便携版 zip + MSI（失败自动跳过）
# 用法：在项目根目录执行  powershell -ExecutionPolicy Bypass -File release.ps1
# 依赖：node/npm（mise 管理）、cargo（rust）、Inno Setup 6（缺失时跳过安装包）

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
if (-not $root) { $root = (Get-Location).Path }

# ---------- 版本号（单一来源：tauri.conf.json） ----------
$conf = Get-Content (Join-Path $root 'src-tauri\tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$version = $conf.version
Write-Host "==== ai-image v$version 打包开始 ====" -ForegroundColor Cyan

$outDir = Join-Path $root 'release'
New-Item -Path $outDir -ItemType Directory -Force | Out-Null

# ---------- 1/5 前端构建 ----------
Write-Host "`n[1/5] 前端构建（vue-tsc + vite）" -ForegroundColor Yellow
Push-Location $root
npm run build
if ($LASTEXITCODE -ne 0) { Pop-Location; throw '前端构建失败' }
Pop-Location

# ---------- 2/5 Rust 编译 ----------
Write-Host "`n[2/5] Rust 编译（cargo build --release）" -ForegroundColor Yellow
Push-Location (Join-Path $root 'src-tauri')
cargo build --release
if ($LASTEXITCODE -ne 0) { Pop-Location; throw 'Rust 编译失败' }
Pop-Location

$exe = Join-Path $root 'src-tauri\target\release\ai-image.exe'
if (-not (Test-Path $exe)) { throw "未找到编译产物：$exe" }

# ---------- 3/5 便携版 ----------
Write-Host "`n[3/5] 便携版打包" -ForegroundColor Yellow
$portableName = "ai-image-v$version-Portable"
$portableDir = Join-Path $outDir $portableName
if (Test-Path $portableDir) { Remove-Item $portableDir -Recurse -Force }
New-Item -Path $portableDir -ItemType Directory -Force | Out-Null
Copy-Item $exe (Join-Path $portableDir 'ai-image.exe') -Force
$readme = @"
AI 图片生成 v$version 便携版
========================================

使用方法：
1. 双击 ai-image.exe 运行
2. 首次使用请点击右上角“设置”，填入 API 供应商地址与 API Key
3. 生成图片后可单张下载或批量下载，保存格式（PNG/JPG）与路径可在设置中调整

运行环境：
- Windows 10 及以上系统（需要 WebView2 运行时，Win10 22H2 及以上一般已内置）
- 若提示缺少 WebView2，请安装：https://developer.microsoft.com/microsoft-edge/webview2/

说明：
- 本软件为绿色免安装版，配置保存在系统用户数据目录（%APPDATA%\com.aiimage.app）
- 直接删除程序目录即可移除软件；如需彻底清除配置，请一并删除上述数据目录
"@
Set-Content -Path (Join-Path $portableDir '使用说明.txt') -Value $readme -Encoding UTF8
$zip = Join-Path $outDir "$portableName.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($portableDir, $zip)
Remove-Item $portableDir -Recurse -Force
Write-Host "  [OK] $portableName.zip"

# ---------- 4/5 Inno Setup 安装包 ----------
Write-Host "`n[4/5] Inno Setup 安装包" -ForegroundColor Yellow
$iscc = $null
$pf86 = ${env:ProgramFiles(x86)}
$candidates = @(
  (Join-Path $pf86 'Inno Setup 6\ISCC.exe'),
  (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
) | Where-Object { $_ -and (Test-Path $_) }
if ($candidates) { $iscc = $candidates[0] }
if (-not $iscc) {
  foreach ($hive in 'HKLM', 'HKCU') {
    $key = Get-ItemProperty "Registry::$hive\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Inno Setup 6_is1" -ErrorAction SilentlyContinue
    if ($key -and $key.InstallLocation) {
      $p = Join-Path $key.InstallLocation 'ISCC.exe'
      if (Test-Path $p) { $iscc = $p; break }
    }
  }
}
if (-not $iscc) {
  $iscc = Get-ChildItem 'C:\Users\*\AppData\Local\Programs\Inno Setup*\ISCC.exe' -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if ($iscc) {
  Write-Host "  ISCC: $iscc"
  & $iscc "/DAppVersion=$version" (Join-Path $root 'src-tauri\windows\ai-image.iss') | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'Inno Setup 编译失败' }
  Write-Host "  [OK] ai-image-v$version-Setup.exe"
} else {
  Write-Warning '未找到 Inno Setup 6，已跳过安装包（可从 https://jrsoftware.org/isinfo.php 安装后重跑）'
}

# ---------- 5/5 产物清单 ----------
# 说明：MSI 打包依赖 WiX 工具集（tauri CLI 从 GitHub 下载），当前网络环境下下载超时，
# 按需求跳过 MSI。如需 MSI：手动安装 WiX 3.14 并解压到 %LOCALAPPDATA%\tauri\WixTools3，
# 然后执行  npx tauri build --bundles msi  即可。

Write-Host "`n==== 打包完成，产物位于 $outDir ====" -ForegroundColor Cyan
Get-ChildItem $outDir -File | Sort-Object LastWriteTime -Descending |
    ForEach-Object { "  {0}  {1:N1} MB" -f $_.Name, ($_.Length / 1MB) }
