param(
  [int]$MaxCandidates = 15,
  [int]$TimeoutSeconds = 4,
  [string]$OutputPath = ".\.run\public-proxy-sample.json"
)
$ErrorActionPreference = "Stop"
$source = "https://raw.githubusercontent.com/TheSpeedX/PROXY-List/master/http.txt"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$out = Join-Path $root $OutputPath
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $out) | Out-Null
$raw = & "$env:SystemRoot\System32\curl.exe" --fail --silent --show-error --location --max-time 20 $source
if($LASTEXITCODE -ne 0) { throw "failed to download public proxy sample" }
$lines = $raw -split "`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ -match '^\d{1,3}(\.\d{1,3}){3}:\d+$' } | Select-Object -First $MaxCandidates
$results = foreach($line in $lines) {
  $proxy = "http://$line"
  $psi = [Diagnostics.ProcessStartInfo]::new()
  $psi.FileName = "$env:SystemRoot\System32\curl.exe"
  $psi.Arguments = "--silent --show-error --output NUL --max-time $TimeoutSeconds --proxy $proxy --write-out `"%{http_code} %{time_total}`" http://example.com/"
  $psi.UseShellExecute = $false
  $psi.CreateNoWindow = $true
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  $process = [Diagnostics.Process]::Start($psi)
  $probe = $process.StandardOutput.ReadToEnd()
  $null = $process.StandardError.ReadToEnd()
  $process.WaitForExit()
  $exitCode = $process.ExitCode
  $parts = "$probe".Trim() -split '\s+'
  $status = if($parts.Count -gt 0 -and $parts[0] -match '^\d+$'){ [int]$parts[0] }else{ 0 }
  $seconds = if($parts.Count -gt 1 -and $parts[1] -match '^\d+(\.\d+)?$'){ [double]$parts[1] }else{ 0 }
  [ordered]@{
    proxy = $proxy
    reachable = ($exitCode -eq 0 -and $status -ge 200 -and $status -lt 500)
    status = $status
    elapsedMs = [math]::Round($seconds * 1000, 1)
    curlExitCode = $exitCode
  }
}
$healthy = @($results | Where-Object reachable)
$summary = [ordered]@{ source=$source; checked=@($results).Count; reachable=$healthy.Count; results=$results }
[IO.File]::WriteAllText($out, ($summary | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
$summary | ConvertTo-Json -Depth 5
