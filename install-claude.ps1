#Requires -Version 5.1
$ErrorActionPreference = "Stop"

Write-Host "=== Claude Code 설치 (Windows) ===" -ForegroundColor Cyan
Write-Host ""

# --- 관리자 권한 체크 ---
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "ERROR: 관리자 권한이 필요합니다." -ForegroundColor Red
    Write-Host ""
    Write-Host "  1. 시작 메뉴에서 'PowerShell' 검색"
    Write-Host "  2. '관리자 권한으로 실행' 클릭"
    Write-Host "  3. 이 스크립트를 다시 실행하세요"
    Write-Host ""
    exit 1
}

# --- ExecutionPolicy 설정 ---
Write-Host "[1/5] 스크립트 실행 권한 설정..."
$currentPolicy = Get-ExecutionPolicy
if ($currentPolicy -eq "Restricted" -or $currentPolicy -eq "AllSigned") {
    Set-ExecutionPolicy RemoteSigned -Scope CurrentUser -Force
    Write-Host "  ExecutionPolicy: RemoteSigned 로 변경 완료"
} else {
    Write-Host "  ExecutionPolicy: OK ($currentPolicy)"
}

# --- winget 확인 ---
function Test-Winget {
    try { winget --version | Out-Null; return $true }
    catch { return $false }
}

$hasWinget = Test-Winget

# --- PATH 갱신 함수 ---
function Refresh-Path {
    $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $env:Path = "$machinePath;$userPath"
}

# --- 2. Node.js ---
Write-Host ""
Write-Host "[2/5] Node.js 확인..."
$hasNode = Get-Command node -ErrorAction SilentlyContinue
if (-not $hasNode) {
    if ($hasWinget) {
        Write-Host "  Node.js 설치 중..."
        winget install OpenJS.NodeJS.LTS --accept-source-agreements --accept-package-agreements
        Refresh-Path
        Write-Host "  Node.js: 설치 완료"
    } else {
        Write-Host "  winget이 없습니다. Node.js를 직접 설치하세요:" -ForegroundColor Yellow
        Write-Host "  https://nodejs.org" -ForegroundColor Yellow
        exit 1
    }
} else {
    Write-Host "  Node.js: OK ($(node --version))"
}

# --- 3. jq ---
Write-Host ""
Write-Host "[3/5] jq 확인..."
$hasJq = Get-Command jq -ErrorAction SilentlyContinue
if (-not $hasJq) {
    if ($hasWinget) {
        Write-Host "  jq 설치 중 (상태바에 필요)..."
        winget install jqlang.jq --accept-source-agreements --accept-package-agreements
        Refresh-Path
        Write-Host "  jq: 설치 완료"
    } else {
        Write-Host "  jq를 직접 설치하세요: https://jqlang.github.io/jq/download/" -ForegroundColor Yellow
    }
} else {
    Write-Host "  jq: OK"
}

# --- 4. Git for Windows ---
Write-Host ""
Write-Host "[4/5] Git 확인..."
$hasGit = Get-Command git -ErrorAction SilentlyContinue
if (-not $hasGit) {
    if ($hasWinget) {
        Write-Host "  Git 설치 중 (hooks 실행에 필요)..."
        winget install Git.Git --accept-source-agreements --accept-package-agreements
        Refresh-Path
        Write-Host "  Git: 설치 완료"
    } else {
        Write-Host "  Git을 직접 설치하세요: https://git-scm.com/download/win" -ForegroundColor Yellow
    }
} else {
    Write-Host "  Git: OK"
}

# --- 5. Claude Code ---
Write-Host ""
Write-Host "[5/5] Claude Code 설치..."
npm install -g @anthropic-ai/claude-code
Write-Host "  Claude Code: 설치 완료"

# --- Done ---
Write-Host ""
Write-Host "=== 설치 완료! ===" -ForegroundColor Green
Write-Host ""
Write-Host "다음 단계:"
Write-Host "  1. 터미널을 재시작하세요 (PATH 적용)"
Write-Host "  2. 'claude' 를 실행하여 로그인하세요"
Write-Host ""
Write-Host "추천 터미널: Windows Terminal (Microsoft Store에서 설치)"
Write-Host ""
