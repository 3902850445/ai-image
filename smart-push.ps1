# smart-push.ps1：探活窗口内智能推送（main --force + v0.3.0 标签）
$repo = 'C:\dev\ai-image'
$maxRounds = 30
$interval = 15

function Test-Gh443 {
    $c = New-Object Net.Sockets.TcpClient
    try {
        $t = $c.BeginConnect('github.com', 443, $null, $null)
        if ($t.AsyncWaitHandle.WaitOne(3000) -and $c.Connected) { return $true } else { return $false }
    } catch {
        return $false
    } finally {
        $c.Close()
    }
}

$mainDone = $false
$tagDone = $false

for ($round = 1; $round -le $maxRounds; $round++) {
    if ($mainDone -and $tagDone) { break }
    $alive = Test-Gh443
    Write-Host ("[轮 {0}] {1} 探活: {2}" -f $round, (Get-Date -Format 'HH:mm:ss'), $(if ($alive) { '通' } else { '断' }))
    if (-not $alive) { Start-Sleep $interval; continue }

    if (-not $mainDone) {
        git -C $repo push github main --force 2>&1 | ForEach-Object { "  $_" }
        if ($LASTEXITCODE -eq 0) {
            $mainDone = $true
            Write-Host '  [OK] main 推送成功' -ForegroundColor Green
        } else {
            Write-Host '  推送失败，回到探活循环'
            Start-Sleep $interval
            continue
        }
    }
    if (-not $tagDone) {
        git -C $repo push github v0.3.0 2>&1 | ForEach-Object { "  $_" }
        if ($LASTEXITCODE -eq 0) {
            $tagDone = $true
            Write-Host '  [OK] 标签 v0.3.0 推送成功，云构建已触发' -ForegroundColor Green
        } else {
            Write-Host '  标签推送失败，回到探活循环'
            Start-Sleep $interval
            continue
        }
    }
}

if ($mainDone -and $tagDone) {
    Write-Host '==== 全部推送完成，远程验证 ====' -ForegroundColor Green
    git -C $repo ls-remote github | ForEach-Object { "  $_" }
    Write-Host '==== 云构建页面: https://github.com/3902850445/ai-image/actions ===='
    exit 0
} else {
    Write-Host '==== 30 轮探活用尽仍有未完成项，重跑本脚本即可继续 ====' -ForegroundColor Red
    exit 1
}
