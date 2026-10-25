param([string]$Python = "python", [string]$EnvironmentDir)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (-not $EnvironmentDir) {
    $workspaceVenv = Join-Path $projectRoot '.venv-math-ocr'
    if (Test-Path -LiteralPath (Join-Path $workspaceVenv 'Scripts\python.exe')) { $EnvironmentDir = $workspaceVenv }
    else { $EnvironmentDir = Join-Path $env:LOCALAPPDATA 'Paperdesk\math-ocr' }
}
$venvRoot = $EnvironmentDir
$cachedModels = Join-Path $projectRoot 'tmp\math-models'
$env:PADDLE_PDX_CACHE_HOME = if (Test-Path -LiteralPath (Join-Path $cachedModels 'official_models')) { $cachedModels } else { Join-Path $venvRoot 'models' }
$env:HF_HOME = Join-Path $env:PADDLE_PDX_CACHE_HOME 'huggingface'
$env:MODELSCOPE_CACHE = Join-Path $env:PADDLE_PDX_CACHE_HOME 'modelscope'
$venvPython = Join-Path $venvRoot 'Scripts\python.exe'
if (-not (Test-Path -LiteralPath $venvPython)) {
    & $Python -m venv $venvRoot
    if ($LASTEXITCODE -ne 0) { throw 'Python 3.10–3.12 is required; specify -Python with its full path.' }
}
& $venvPython -m pip install -r (Join-Path $projectRoot 'services\math-ocr\requirements.txt')
if ($LASTEXITCODE -ne 0) { throw 'OCR dependency installation failed.' }
Write-Host 'Keep this console running. After Ready appears, open Paperdesk Settings > OCR services and save http://127.0.0.1:8850/layout-parsing.'
& $venvPython (Join-Path $projectRoot 'services\math-ocr\server.py')
