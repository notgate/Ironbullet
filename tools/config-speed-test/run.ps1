param(
  [string]$Ironbullet = "..\..\target\release\ironbullet.exe",
  [int]$Requests = 1000,
  [int]$Threads = 100,
  [int]$DelayMs = 10,
  [int]$Port = 18787,
  [int]$ProxyPort = 18788,
  [switch]$UseLocalProxy,
  [string]$ResultPath = ""
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = (Resolve-Path (Join-Path $root "..\..")).Path
$exe = (Resolve-Path (Join-Path $root $Ironbullet)).Path
$work = Join-Path $root ".run"
New-Item -ItemType Directory -Force -Path $work | Out-Null
$stdout = Join-Path $work "server.stdout.log"
$stderr = Join-Path $work "server.stderr.log"
$serverToken = [Guid]::NewGuid().ToString('N')
$server = Start-Process node -ArgumentList @((Join-Path $root "server.mjs"), "--port", $Port, "--proxy-port", $ProxyPort, "--token", $serverToken) -WorkingDirectory $root -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
try {
  $health = "http://127.0.0.1:$Port/health"
  $ready = $false
  for($i=0; $i -lt 80; $i++) {
    try {
      $server.Refresh()
      if($server.HasExited) { throw "speed target exited with code $($server.ExitCode); see $stderr" }
      $healthResponse = Invoke-RestMethod -Uri $health -TimeoutSec 1
      if($healthResponse.token -eq $serverToken) { $ready = $true; break }
    } catch {
      if($server.HasExited) { throw }
    }
    Start-Sleep -Milliseconds 100
  }
  if(-not $ready) { throw "speed target did not start; see $stderr" }
  Invoke-RestMethod -Uri "http://127.0.0.1:$Port/api/reset" | Out-Null

  $config = (Get-Content (Join-Path $root "speed-test.rfx") -Raw).Replace("__TARGET_URL__", "http://127.0.0.1:$Port").Replace("__DELAY_MS__", "$DelayMs")
  if($UseLocalProxy) { $config = $config -replace '"proxy_mode"\s*:\s*"None"', '"proxy_mode":"Rotate"' }
  $configPath = Join-Path $work "speed-test.generated.rfx"
  [IO.File]::WriteAllText($configPath, $config, [Text.UTF8Encoding]::new($false))
  $wordlist = Join-Path $work "wordlist.txt"
  0..($Requests - 1) | ForEach-Object { "request-$($_.ToString('D8')):fixture" } | Set-Content -LiteralPath $wordlist -Encoding ascii
  if($UseLocalProxy) {
    $proxyFile = Join-Path $work "loopback-proxy.txt"
    "http://127.0.0.1:$ProxyPort" | Set-Content -LiteralPath $proxyFile -Encoding ascii
  }

  $runnerOutput = Join-Path $work ($(if($UseLocalProxy){"runner-proxy.stdout.log"}else{"runner-direct.stdout.log"}))
  $runnerError = Join-Path $work ($(if($UseLocalProxy){"runner-proxy.stderr.log"}else{"runner-direct.stderr.log"}))
  $runnerArgs = "--config `"$configPath`" --wordlist `"$wordlist`" --threads $Threads"
  if($UseLocalProxy) { $runnerArgs += " --proxies `"$proxyFile`"" }
  $timer = [Diagnostics.Stopwatch]::StartNew()
  $runner = Start-Process -FilePath $exe -ArgumentList $runnerArgs -WorkingDirectory (Split-Path $exe -Parent) -RedirectStandardOutput $runnerOutput -RedirectStandardError $runnerError -Wait -PassThru
  $exitCode = $runner.ExitCode
  $timer.Stop()
  $stats = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/api/stats"
  $target = $stats.target
  $missing = $Requests - [int]$target.uniqueRequests
  $summaryText = Get-Content -LiteralPath $runnerError -Raw
  $summaryMatch = [regex]::Match($summaryText, 'done in [^\r\n]*?([0-9]+) processed, ([0-9]+) hits, ([0-9]+) fails, ([0-9]+) errors')
  if(!$summaryMatch.Success) { throw "IronBullet final counters were not found in $runnerError" }
  $runnerProcessed = [int]$summaryMatch.Groups[1].Value
  $runnerHits = [int]$summaryMatch.Groups[2].Value
  $runnerFails = [int]$summaryMatch.Groups[3].Value
  $runnerErrors = [int]$summaryMatch.Groups[4].Value

  $failures = @()
  if($exitCode -ne 0) { $failures += "runner exit code $exitCode" }
  if($runnerProcessed -ne $Requests) { $failures += "runner processed $runnerProcessed of $Requests" }
  if($runnerHits -ne $Requests) { $failures += "runner completed $runnerHits successful responses of $Requests" }
  if($runnerFails -ne 0) { $failures += "runner reported $runnerFails fails" }
  if($runnerErrors -ne 0) { $failures += "runner reported $runnerErrors errors" }
  if($missing -ne 0) { $failures += "$missing target requests missing" }
  if([int]$target.duplicateRequests -ne 0) { $failures += "$($target.duplicateRequests) duplicate target requests" }
  if([int]$target.requests -ne $Requests) { $failures += "target received $($target.requests) of $Requests requests" }
  if($Threads -gt 1 -and [int]$target.maxActive -le 1) { $failures += "runner serialized requests unexpectedly" }
  if($UseLocalProxy -and [int]$stats.proxy.requests -ne $Requests) { $failures += "proxy received $($stats.proxy.requests) of $Requests requests" }

  $result = [ordered]@{
    status = $(if($failures.Count -eq 0){"PASS"}else{"FAIL"})
    mode = $(if($UseLocalProxy){"loopback-proxy"}else{"direct"})
    requests = $Requests
    threads = $Threads
    delayMs = $DelayMs
    processExitCode = $exitCode
    runnerProcessed = $runnerProcessed
    runnerHits = $runnerHits
    runnerFails = $runnerFails
    runnerErrors = $runnerErrors
    elapsedMs = [math]::Round($timer.Elapsed.TotalMilliseconds, 1)
    successfulRequestsPerSecond = [math]::Round($runnerHits / $timer.Elapsed.TotalSeconds, 1)
    targetRequests = [int]$target.requests
    uniqueRequests = [int]$target.uniqueRequests
    duplicateRequests = [int]$target.duplicateRequests
    missingRequests = $missing
    targetMaxActive = [int]$target.maxActive
    proxyRequests = [int]$stats.proxy.requests
    proxyMaxActive = [int]$stats.proxy.maxActive
    failures = $failures
  }
  $json = $result | ConvertTo-Json
  $json | Write-Output
  if($ResultPath) { [IO.File]::WriteAllText($ResultPath, $json, [Text.UTF8Encoding]::new($false)) }
  if($failures.Count -ne 0) { throw "benchmark integrity check failed: $($failures -join '; ')" }
} finally {
  if($server -and -not $server.HasExited) { Stop-Process -Id $server.Id -Force }
}
