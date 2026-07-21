param(
  [string]$Bundle = "..\..\dist\ironbullet-v0.6.2-rc.5-windows-x64",
  [int]$Port = 18987
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$bundlePath = (Resolve-Path (Join-Path $root $Bundle)).Path
$exe = Join-Path $bundlePath "ironbullet.exe"
if(!(Test-Path (Join-Path $bundlePath "xray.exe"))) { throw "bundle has no xray.exe" }
$work = Join-Path $root ".run\xray-cleanup"
New-Item -ItemType Directory -Force -Path $work | Out-Null
$serverOut = Join-Path $work "server.out.log"
$serverErr = Join-Path $work "server.err.log"
$serverToken = [Guid]::NewGuid().ToString('N')
$server = Start-Process node -ArgumentList @((Join-Path $root "server.mjs"), "--port", $Port, "--proxy-port", ($Port + 1), "--token", $serverToken) -WorkingDirectory $root -WindowStyle Hidden -RedirectStandardOutput $serverOut -RedirectStandardError $serverErr -PassThru
try {
  $ready = $false
  for($i=0; $i -lt 80; $i++) {
    try {
      $server.Refresh()
      if($server.HasExited) { throw "loopback target exited with code $($server.ExitCode); see $serverErr" }
      $health = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/health" -TimeoutSec 1
      if($health.token -eq $serverToken) { $ready=$true; break }
    } catch {
      if($server.HasExited) { throw }
    }
    Start-Sleep -Milliseconds 100
  }
  if(!$ready) { throw "loopback target did not start" }

  $config = (Get-Content (Join-Path $root "speed-test.rfx") -Raw).Replace("__TARGET_URL__", "http://127.0.0.1:$Port").Replace("__DELAY_MS__", "0")
  $config = $config -replace '"proxy_mode"\s*:\s*"None"', '"proxy_mode":"Rotate"'
  $configPath = Join-Path $work "xray-cleanup.rfx"
  [IO.File]::WriteAllText($configPath, $config, [Text.UTF8Encoding]::new($false))
  "request-00000001:fixture" | Set-Content (Join-Path $work "wordlist.txt") -Encoding ascii
  # TEST-NET-3 is intentionally non-routable; Xray still opens its local SOCKS
  # listener, keeping the managed child alive long enough to observe it.
  "vless://123e4567-e89b-12d3-a456-426614174000@203.0.113.1:443?encryption=none&security=none&type=tcp" | Set-Content (Join-Path $work "proxies.txt") -Encoding ascii

  $before = @(Get-Process xray -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
  $xrayConfigDir = Join-Path $env:LOCALAPPDATA "Ironbullet\xray"
  $configsBefore = @(Get-ChildItem $xrayConfigDir -Filter *.json -File -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName)
  $psi = New-Object System.Diagnostics.ProcessStartInfo
  $psi.FileName = $exe
  $psi.Arguments = '--config "{0}" --wordlist "{1}" --proxies "{2}" --threads 1' -f $configPath, (Join-Path $work "wordlist.txt"), (Join-Path $work "proxies.txt")
  $psi.WorkingDirectory = $bundlePath
  $psi.UseShellExecute = $false
  $psi.CreateNoWindow = $true
  $runner = New-Object System.Diagnostics.Process
  $runner.StartInfo = $psi
  if(!$runner.Start()) { throw "IronBullet process did not start" }

  $observed = @()
  for($i=0; $i -lt 100 -and !$runner.HasExited; $i++) {
    $observed = @(Get-Process xray -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.Id } | Select-Object -ExpandProperty Id)
    if($observed.Count -gt 0) { break }
    Start-Sleep -Milliseconds 100
  }
  if($observed.Count -eq 0) { throw "xray.exe was never observed; smoke test did not exercise the adapter" }

  if(!$runner.WaitForExit(30000)) {
    Stop-Process -Id $runner.Id -Force -ErrorAction SilentlyContinue
    throw "IronBullet did not exit within 30 seconds"
  }
  $runner.WaitForExit()
  $runner.Refresh()
  $exitCode = $runner.ExitCode
  Start-Sleep -Milliseconds 400
  $after = @(Get-Process xray -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.Id } | Select-Object -ExpandProperty Id)
  $configsAfter = @(Get-ChildItem $xrayConfigDir -Filter *.json -File -ErrorAction SilentlyContinue | Where-Object { $configsBefore -notcontains $_.FullName } | Select-Object -ExpandProperty FullName)
  $result = [ordered]@{ ironbulletExitCode=$exitCode; observedXrayPids=$observed; leakedXrayPids=$after; leakedConfigPaths=$configsAfter }
  $result | ConvertTo-Json | Write-Output
  if($exitCode -ne 0) { throw "IronBullet exited with code $exitCode" }
  if($after.Count -ne 0) { throw "xray process leak detected: $($after -join ', ')" }
  if($configsAfter.Count -ne 0) { throw "xray config leak detected: $($configsAfter -join ', ')" }
} finally {
  if($server -and !$server.HasExited) { Stop-Process -Id $server.Id -Force }
}
