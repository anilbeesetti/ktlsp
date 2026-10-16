# Shared native CI preflight for #329; no cause of the old failure is assumed.
# Retain bounded original PowerShell stdout/error facts and actual guard operands.
# Complementary byte captures use a different invocation and cannot override them.
param(
    [Parameter(Mandatory)] [string] $EvidenceDirectory
)

$ErrorActionPreference = 'Stop'
$limit = 65536
New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
$facts = [ordered]@{
    source_sha = $env:SOURCE_SHA
    powershell = $PSVersionTable.PSVersion.ToString()
    capture_mechanism = 'Original PowerShell native argv/output path with bounded collector; complementary ProcessStartInfo raw queries are separate observations'
    working_directory = (Get-Location).ProviderPath
    native_argument_passing = "$PSNativeCommandArgumentPassing"
    native_error_action_preference = $PSNativeCommandUseErrorActionPreference
    console_output_encoding = [Console]::OutputEncoding.WebName
    powershell_output_encoding = $OutputEncoding.WebName
    stream_limit_bytes = $limit
    command_timeout_ms = 30000
    commands = [ordered]@{}
    powershell_commands = [ordered]@{}
}
$record = Join-Path $EvidenceDirectory 'source-preflight.json'
function Save-Facts {
    $facts | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $record -Encoding utf8NoBOM
}
function Invoke-PowerShellFact([ValidateSet('checkout', 'head', 'tree', 'status')] [string] $Name) {
    $output = [System.Collections.Generic.List[string]]::new()
    $errors = [System.Collections.Generic.List[string]]::new()
    $sizes = [long[]]@(0, 0)
    $command = [ordered]@{
        executable = 'git'
        argv = @(switch ($Name) {
            'checkout' { @('-c', 'core.autocrlf=false', '-c', 'core.eol=lf', 'checkout-index', '--all', '--force') }
            'head' { @('rev-parse', 'HEAD') }
            'tree' { @('rev-parse', 'HEAD^{tree}') }
            'status' { @('status', '--porcelain=v1', '--untracked-files=all') }
        })
        stdout = $output
        rendered_errors = $errors
        exit_code = $null
        exception = $null
        overflow = $false
        stream_representation = 'Exact PowerShell strings, without line terminators; errors are rendered PowerShell error records, not native stderr bytes'
    }
    $facts.powershell_commands[$Name] = $command
    Save-Facts
    $script:nativeCode = $null
    $script:nativeCompleted = $false
    try {
        & {
            # Preserve original literal native argument binding and stdout
            # object conversion; no decoding of auxiliary raw files drives guard.
            switch ($Name) {
                'checkout' { git -c core.autocrlf=false -c core.eol=lf checkout-index --all --force }
                'head' { git rev-parse HEAD }
                'tree' { git rev-parse 'HEAD^{tree}' }
                'status' { git status --porcelain=v1 --untracked-files=all }
            }
            # Only normal native completion confirms this command's exit.
            $script:nativeCode = $LASTEXITCODE
            $script:nativeCompleted = $true
        } 2>&1 | ForEach-Object {
            $isError = $_ -is [System.Management.Automation.ErrorRecord]
            $i = if ($isError) { 1 } else { 0 }
            $text = $_.ToString()
            # Charge a separator even for empty lines, bounding record count too.
            $sizes[$i] += [System.Text.Encoding]::UTF8.GetByteCount($text) + 1
            if ($sizes[$i] -gt $limit) {
                $command.overflow = $true
                throw 'Original PowerShell observation exceeded retained evidence bound'
            }
            if ($isError) { $errors.Add($text) } else { $output.Add($text) }
        }
        $command.exit_code = $script:nativeCode
    } catch {
        if ($script:nativeCompleted -and -not $command.overflow) { $command.exit_code = $script:nativeCode }
        else { $command.exit_code = $null }
        # An interruption may leave the preceding command's LASTEXITCODE.
        # Retain it only as an explicitly unconfirmed observation, never exit.
        $command.unconfirmed_last_exitcode_observation = $LASTEXITCODE
        $message = $_.Exception.Message
        $command.exception = $message.Substring(0, [Math]::Min($message.Length, 4096))
        $command.exception_type = $_.Exception.GetType().FullName
    } finally {
        $command.native_completion_confirmed = ($script:nativeCompleted -and -not $command.overflow)
        $command.stdout_text_budget_bytes_including_separators = $sizes[0]
        $command.error_text_budget_bytes_including_separators = $sizes[1]
        Save-Facts
    }
    if ($command.exit_code -ne 0 -or $null -ne $command.exception -or $command.overflow) {
        throw "Original PowerShell source command $Name failed or exceeded retained evidence bounds"
    }
    $output.ToArray()
}
function Invoke-SourceFact([ValidateSet('head', 'tree', 'status')] [string] $Name) {
    # Complementary read-only queries only; do not rematerialize checkout after
    # the original observations. These are NOT original-invocation raw bytes.
    $arguments = switch ($Name) {
        'head' { @('rev-parse', 'HEAD') }
        'tree' { @('rev-parse', 'HEAD^{tree}') }
        'status' { @('status', '--porcelain=v1', '--untracked-files=all') }
    }
    $start = [System.Diagnostics.ProcessStartInfo]::new('git')
    $start.WorkingDirectory = (Get-Location).ProviderPath
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in $arguments) { $start.ArgumentList.Add($argument) }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $start
    $outputs = @($null, $null)
    $observed = [long[]]@(0, 0)
    $retained = [int[]]@(0, 0)
    $ended = @($false, $false)
    $command = [ordered]@{ argv = @($arguments); exit_code = $null; exception = $null; rejection = $null }
    $facts.commands[$Name] = $command
    Save-Facts
    try {
        $outputs[0] = [System.IO.File]::Create((Join-Path $EvidenceDirectory "auxiliary-$Name.stdout"))
        $outputs[1] = [System.IO.File]::Create((Join-Path $EvidenceDirectory "auxiliary-$Name.stderr"))
        if (-not $process.Start()) { throw 'Native process did not start' }
        $clock = [System.Diagnostics.Stopwatch]::StartNew()
        $streams = @($process.StandardOutput.BaseStream, $process.StandardError.BaseStream)
        $buffers = @([byte[]]::new(8192), [byte[]]::new(8192))
        $reads = @($streams[0].ReadAsync($buffers[0], 0, 8192), $streams[1].ReadAsync($buffers[1], 0, 8192))
        while ($null -ne $reads[0] -or $null -ne $reads[1]) {
            if ($clock.ElapsedMilliseconds -ge 30000) {
                $command.rejection = 'command_timeout'
                break
            }
            $pending = [System.Threading.Tasks.Task[]]@($reads | Where-Object { $null -ne $_ })
            [void][System.Threading.Tasks.Task]::WaitAny($pending, 1000)
            for ($i = 0; $i -lt 2; $i++) {
                if ($null -eq $reads[$i] -or -not $reads[$i].IsCompleted) { continue }
                $count = $reads[$i].GetAwaiter().GetResult()
                if ($count -eq 0) {
                    $ended[$i] = $true
                    $reads[$i] = $null
                    continue
                }
                $observed[$i] += $count
                $keep = [Math]::Min($count, $limit - $retained[$i])
                $outputs[$i].Write($buffers[$i], 0, $keep)
                $retained[$i] += $keep
                if ($observed[$i] -gt $limit) { $command.rejection = 'stream_limit' }
                $reads[$i] = $streams[$i].ReadAsync($buffers[$i], 0, 8192)
            }
            if ($null -ne $command.rejection) { break }
        }
        if ($null -eq $command.rejection) {
            $remaining = [int][Math]::Max(0, 30000 - $clock.ElapsedMilliseconds)
            if (-not $process.WaitForExit($remaining)) { $command.rejection = 'command_timeout' }
        }
    } catch {
        $message = $_.Exception.Message
        $command.exception = $message.Substring(0, [Math]::Min($message.Length, 4096))
    } finally {
        try {
            if ($null -ne $process.Id) {
                if (-not $process.HasExited) { $process.Kill($true) }
                if ($process.WaitForExit(5000)) { $command.exit_code = $process.ExitCode }
                else { $command.rejection = 'cleanup_timeout' }
            }
        } catch {
            $message = $_.Exception.Message
            $command.exception = $message.Substring(0, [Math]::Min($message.Length, 4096))
        }
        for ($i = 0; $i -lt 2; $i++) {
            if ($null -ne $outputs[$i]) { $outputs[$i].Dispose() }
        }
        $command.stdout = @{ observed_bytes = $observed[0]; retained_bytes = $retained[0]; complete = $ended[0] }
        $command.stderr = @{ observed_bytes = $observed[1]; retained_bytes = $retained[1]; complete = $ended[1] }
        $process.Dispose()
        Save-Facts
    }
    if ($command.exit_code -ne 0 -or $null -ne $command.exception -or $null -ne $command.rejection -or -not $ended[0] -or -not $ended[1]) {
        throw "Source command $Name failed or exceeded retained evidence bounds"
    }
}
Save-Facts
try {
    Invoke-PowerShellFact 'checkout'
    $head = Invoke-PowerShellFact 'head'
    $tree = Invoke-PowerShellFact 'tree'
    $status = @(Invoke-PowerShellFact 'status')
    $facts.operands = @{
        head = @($head)
        tree = @($tree)
        status = $status
        head_is_null = ($null -eq $head)
        tree_is_null = ($null -eq $tree)
        head_type = if ($null -eq $head) { $null } else { $head.GetType().FullName }
        tree_type = if ($null -eq $tree) { $null } else { $tree.GetType().FullName }
        status_type = $status.GetType().FullName
        head_count = if ($null -eq $head) { 0 } else { @($head).Count }
        tree_count = if ($null -eq $tree) { 0 } else { @($tree).Count }
        status_count = $status.Count
        head_mismatch = [bool]($head -cne $env:SOURCE_SHA)
        tree_invalid = [bool]($tree -cnotmatch '^[0-9a-f]{40}$')
        status_dirty = ($status.Count -ne 0)
    }
    Save-Facts
    # Preserve original operands before auxiliary read-only queries; auxiliary
    # success never replaces an original failure or supplies guard operands.
    Invoke-SourceFact 'head'
    Invoke-SourceFact 'tree'
    Invoke-SourceFact 'status'
    # Original rejection predicate is intentionally unchanged. Do not infer a
    # line-ending or shell cause before the actual operands have been observed.
    if ($head -cne $env:SOURCE_SHA -or $tree -cnotmatch '^[0-9a-f]{40}$' -or $status.Count -ne 0) {
        throw 'Source identity or clean checkout mismatch'
    }
    if (@($head).Count -ne 1 -or @($tree).Count -ne 1 -or $env:SOURCE_SHA -cnotmatch '^[0-9a-f]{40}$') {
        throw 'Source identity must consist of exactly one immutable commit and tree'
    }
    $tree | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'source-tree.txt') -Encoding utf8NoBOM
} catch {
    $message = $_.Exception.Message
    $facts.exception = $message.Substring(0, [Math]::Min($message.Length, 4096))
    Save-Facts
    throw
}
