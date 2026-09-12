# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 baibai and Botting contributors
# This program is free software under GNU AGPL version 3, WITHOUT ANY WARRANTY.
# See LICENSE for the complete terms and Corresponding Source obligations.

<#
.SYNOPSIS
Checks local release materials and refuses public-release preflight for private repositories.
.DESCRIPTION
Reads GitHub metadata through gh. Does not push, publish, change visibility, or create a release.
.OUTPUTS
Exits with code 0 when checks pass, otherwise 1. Reports paths and reasons, never file contents.
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$failures = [System.Collections.Generic.List[string]]::new()

Push-Location -LiteralPath $projectRoot
try {
    foreach ($command in @('git', 'gh')) {
        if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
            throw "Required command is unavailable: $command"
        }
    }

    $requiredFiles = @(
        'LICENSE', 'README.md', 'README.en.md', 'CONTRIBUTING.md',
        'THIRD_PARTY_NOTICES.md', '.github/ISSUE_TEMPLATE/bug_report.yml',
        '.github/ISSUE_TEMPLATE/feature_request.yml'
    )
    foreach ($path in $requiredFiles) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            $failures.Add("Missing release document: $path")
        }
    }
    if (Test-Path -LiteralPath 'LICENSE' -PathType Leaf) {
        $license = Get-Content -Raw -LiteralPath 'LICENSE'
        if ($license -notmatch 'GNU AFFERO GENERAL PUBLIC LICENSE' -or
            $license -notmatch '13\. Remote Network Interaction' -or
            $license -notmatch 'END OF TERMS AND CONDITIONS') {
            $failures.Add('LICENSE does not contain the expected complete AGPLv3 sections.')
        }
    }

    foreach ($path in @('package.json', 'apps/user-desktop/package.json')) {
        $package = Get-Content -Raw -LiteralPath $path | ConvertFrom-Json
        if ($package.private -ne $true -or $package.license -ne 'AGPL-3.0-only') {
            $failures.Add("Package license or npm publication guard is invalid: $path")
        }
    }

    $trackedPaths = @(git ls-files)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot enumerate tracked files.' }
    foreach ($path in $trackedPaths) {
        $name = Split-Path -Leaf $path
        if (($name -like '.env*' -and $name -ne '.env.example') -or
            $name -match '\.(pem|key|p12|pfx)$' -or
            $name -match '^(cookies.*\.txt|tokens.*\.json|accounts-export.*\.json)$' -or
            $path -match '(^|/)(node_modules|target|dist|session-logs)/' -or
            $name -like '*.log') {
            $failures.Add("Local data or build output is tracked: $path")
        }
    }

    $sourcePaths = @(git ls-files --cached --others --exclude-standard -- apps crates)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot enumerate source files.' }
    $checkedSources = 0
    foreach ($path in ($sourcePaths | Sort-Object -Unique)) {
        if ($path -notmatch '\.(rs|ts|svelte|css|html)$' -or
            $path -match '(^|/)(tests|gen)/' -or
            -not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
        $header = (Get-Content -LiteralPath $path -TotalCount 20) -join "`n"
        if ($header -notmatch 'SPDX-License-Identifier: AGPL-3.0-only') {
            $failures.Add("Source license header is missing: $path")
        }
        $checkedSources++
    }

    $changes = @(git status --porcelain)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect worktree status.' }
    if ($changes.Count -gt 0) {
        $failures.Add('Worktree is not clean. Review and commit the intended release before preflight.')
    }

    $origin = git remote get-url origin
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($origin)) {
        throw 'Cannot resolve origin. A verified GitHub repository is required.'
    }
    $metadataJson = gh repo view $origin --json nameWithOwner,visibility
    if ($LASTEXITCODE -ne 0) { throw 'Cannot verify GitHub visibility. Authenticate gh and retry.' }
    $repository = $metadataJson | ConvertFrom-Json
    if ($repository.visibility -ne 'PUBLIC') {
        $failures.Add("Public release blocked: $($repository.nameWithOwner) is $($repository.visibility).")
    }

    Write-Output "Checked release documents, package guards, tracked paths, $checkedSources source headers, and GitHub visibility."
    if ($failures.Count -gt 0) {
        foreach ($failure in $failures) { Write-Output "BLOCKED: $failure" }
        exit 1
    }
    Write-Output 'Public-release preflight passed. No publication was performed.'
} catch {
    Write-Output "BLOCKED: $($_.Exception.Message)"
    exit 1
} finally {
    Pop-Location
}
