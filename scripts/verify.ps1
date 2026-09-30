# verify.ps1 -- Kanesumi one-shot verification (Windows side): full test run + clippy baseline diff.
#
# Why it exists: this repo carries PRE-EXISTING clippy warnings (CJK comments are measured
# double-width, so some lines wrap), and the rule is "no NEW warnings". Comparing by hand is
# easy to get wrong, so this script freezes the comparison: it extracts the per-target
# "generated N warnings" summary and diffs it against scripts/clippy-baseline.txt.
# scripts/verify.sh prints the same shape and shares this baseline file (both honour @<os> tags).
#
# !! THIS FILE MUST STAY PURE ASCII !!
# Windows PowerShell 5.1 reads a BOM-less UTF-8 script as GBK; a CJK comment can then eat the
# following newline (an invalid GBK pair swallows the LF), which merges the next code line into
# the comment and breaks parsing. ASCII is immune. The Chinese rationale lives in
# docs/WINDOWS_RESEARCH_BACKLOG.md section D2.
#
# Usage:
#   pwsh scripts/verify.ps1                    # tests + clippy baseline diff
#   pwsh scripts/verify.ps1 -UpdateBaseline    # overwrite the baseline with the current result

[CmdletBinding()]
param([switch]$UpdateBaseline)

$ErrorActionPreference = 'Continue'
Push-Location (Join-Path $PSScriptRoot '..')
$failed = 0
$baselinePath = 'scripts/clippy-baseline.txt'
# Platform tag: lines tagged with another platform are ignored (see verify.sh for the rationale --
# kanesumi-harness' Wayland+wgpu shell is cfg-gated out off-Linux, so the warning sets differ).
$osTag = if ($env:OS -eq 'Windows_NT' -or $IsWindows) { 'windows' } elseif ($IsMacOS) { 'darwin' } elseif ($IsLinux) { 'linux' } else { 'unknown' }

Write-Host '== 1/3 tests (cargo test --workspace) =='
$testOut = cargo test --workspace 2>&1
$passed = 0
foreach ($m in ($testOut | Select-String -Pattern 'test result: ok\. (\d+) passed')) {
    $passed += [int]$m.Matches[0].Groups[1].Value
}
$failedSuites = ($testOut | Select-String -Pattern 'test result: FAILED').Count
if ($failedSuites -ne 0) {
    Write-Host "[FAIL] $failedSuites test target(s) failed:" -ForegroundColor Red
    $testOut | Select-String -Pattern 'test result: FAILED|FAILED$' | Select-Object -First 20 | ForEach-Object { $_.Line }
    $failed = 1
} else {
    Write-Host "[OK] all passed, $passed checks total" -ForegroundColor Green
}

Write-Host ''
Write-Host '== 2/3 test font availability =='
$candidates = @(
    $env:KANESUMI_TEST_FONT,
    'C:/Windows/Fonts/msyh.ttc',
    'C:/Windows/Fonts/segoeui.ttf',
    '/usr/local/share/fonts/s/SourceHanSansSC-Regular.otf',
    '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'
) | Where-Object { $_ }
$found = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($found) {
    Write-Host "[OK] test font: $found" -ForegroundColor Green
} else {
    Write-Host '[WARN] no test font found: font-dependent tests are SILENTLY SKIPPED (set KANESUMI_TEST_FONT)' -ForegroundColor Yellow
    $failed = 1
}

Write-Host ''
Write-Host '== 3/3 clippy vs baseline (cargo clippy --workspace --all-targets) =='

function Get-ClippySummary {
    param([string[]]$Output)
    # Real shape:  warning: `kanesumi-core` (lib test) generated 1 warning (run ...)
    # Deliberately NO backtick in the pattern: the target name is captured including the
    # backticks and the "(kind)" suffix, and the baseline file stores exactly that string --
    # generator and comparer agree by construction. (An earlier pattern expected "generated"
    # right after the closing backtick and therefore matched nothing at all.)
    return $Output |
        Select-String -Pattern '^warning: (.+) generated (\d+) warning' |
        ForEach-Object { '{0}  {1}' -f $_.Matches[0].Groups[1].Value, $_.Matches[0].Groups[2].Value } |
        Sort-Object
}

function Get-BaselineForOs {
    param([string]$Path, [string]$OsTag)
    $out = New-Object System.Collections.Generic.List[string]
    foreach ($line in (Get-Content -LiteralPath $Path)) {
        # Normalise first: the baseline may carry a UTF-8 BOM / CRLF from a PS 5.1 write.
        $l = $line.TrimStart([char]0xFEFF).Trim()
        if ($l -eq '' -or $l.StartsWith('#')) { continue }
        if ($l -match '@([A-Za-z]+)\s*$') {
            if ($Matches[1] -ne $OsTag) { continue }
            $l = ($l -replace '@[A-Za-z]+\s*$', '').TrimEnd()
        }
        $out.Add($l)
    }
    return $out.ToArray()
}

$clippyOut = cargo clippy --workspace --all-targets 2>&1
$currentLines = Get-ClippySummary -Output $clippyOut

# clippy does NOT re-print warnings on a cache hit (it only prints Finished), which would
# silently write an empty baseline. Detection: zero summary lines -> clean this workspace's
# own crates and re-run once, which guarantees a fresh compile and fresh diagnostics.
if (-not $currentLines) {
    Write-Host '(clippy printed nothing -> cache hit; cleaning own crates to harvest real warnings)'
    # NOTE: cargo rejects comma-separated names ("invalid character `,`") -- one -p per crate.
    $own = Get-ChildItem -Path . -Directory -Filter 'kanesumi-*' | Select-Object -ExpandProperty Name
    foreach ($c in $own) { & cargo clean -p $c 2>&1 | Out-Null }
    $clippyOut = cargo clippy --workspace --all-targets 2>&1
    $currentLines = @(Get-ClippySummary -Output $clippyOut)
}

if ($UpdateBaseline) {
    # Carry over platform tags by TARGET (not by count, or a changed count would drop the tag);
    # a target that never had a tag stays platform-neutral.
    $wasTag = @{}
    if (Test-Path $baselinePath) {
        foreach ($line in (Get-Content -LiteralPath $baselinePath)) {
            $l = $line.TrimStart([char]0xFEFF).Trim()
            if ($l -eq '' -or $l.StartsWith('#')) { continue }
            $tag = ''
            if ($l -match '@([A-Za-z]+)\s*$') { $tag = '@' + $Matches[1]; $l = ($l -replace '@[A-Za-z]+\s*$', '').TrimEnd() }
            $lbl = ($l -replace '\s+\d+$', '')
            if ($tag -ne '') { $wasTag[$lbl] = $tag }
        }
    }
    $write = New-Object System.Collections.Generic.List[string]
    $write.Add('# clippy warning baseline: "<target>  <count>", optional "@<os>" suffix limits a line to one platform.')
    $write.Add('# Tags are carried over by -UpdateBaseline (matched by target, not by count).')
    foreach ($l in @($currentLines)) {
        $lbl = ($l -replace '\s+\d+$', '')
        if ($wasTag.ContainsKey($lbl)) { $write.Add($l + '  ' + $wasTag[$lbl]) } else { $write.Add($l) }
    }
    # Write WITHOUT a BOM: PS 5.1's "Set-Content -Encoding UTF8" adds one, which makes the
    # byte-wise diff in verify.sh never match on its first line.
    $full = Join-Path (Get-Location).Path $baselinePath
    [System.IO.File]::WriteAllLines($full, $write.ToArray(), (New-Object System.Text.UTF8Encoding($false)))
    Write-Host "baseline written to $baselinePath ($($write.Count) lines)"
    $write | ForEach-Object { "  $_" }
    Pop-Location
    exit 0
}

if (-not (Test-Path $baselinePath)) {
    Write-Host "[WARN] baseline file $baselinePath missing -- run with -UpdateBaseline first" -ForegroundColor Yellow
    $failed = 1
} else {
    # NOTE: an empty baseline file makes Get-Content return $null, and Compare-Object then
    # throws (or, worse, is skipped) -- normalise both sides to arrays first.
    $baseLines = @(Get-BaselineForOs -Path $baselinePath -OsTag $osTag)
    $currentArr = @($currentLines)
    $diff = @(Compare-Object -ReferenceObject $baseLines -DifferenceObject $currentArr)
    if ($diff.Count -gt 0) {
        Write-Host '[FAIL] clippy warnings differ from baseline (=> added / <= removed):' -ForegroundColor Red
        $diff | ForEach-Object { '  {0} {1}' -f $_.SideIndicator, $_.InputObject }
        $failed = 1
    } elseif ($currentArr.Count -eq 0) {
        Write-Host '[WARN] baseline is empty AND clippy emitted nothing -- cannot certify (run -UpdateBaseline after a clean build)' -ForegroundColor Yellow
        $failed = 1
    } else {
        Write-Host "[OK] warn counts match baseline line by line ($($currentArr.Count) targets)" -ForegroundColor Green
    }
}

Write-Host ''
if ($failed -eq 0) { Write-Host '== verification passed ==' -ForegroundColor Green } else { Write-Host '== verification FAILED (see above) ==' -ForegroundColor Red }
Pop-Location
exit $failed
