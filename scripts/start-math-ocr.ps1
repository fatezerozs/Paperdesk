param([string]$Python = "python", [string]$EnvironmentDir)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (-not $EnvironmentDir) { $EnvironmentDir = Join-Path $env:LOCALAPPDATA 'Paperdesk\math-ocr' }
$venvRoot = $EnvironmentDir
$env:PADDLE_PDX_CACHE_HOME = Join-Path $venvRoot 'models'
$env:HF_HOME = Join-Path $venvRoot 'huggingface'
$env:MODELSCOPE_CACHE = Join-Path $venvRoot 'modelscope'
$venvPython = Join-Path $venvRoot 'Scripts\python.exe'
if (-not (Test-Path -LiteralPath $venvPython)) {
    & $Python -m venv $venvRoot
    if ($LASTEXITCODE -ne 0) { throw 'Python 3.10–3.12 is required; specify -Python with its full path.' }
}
& $venvPython -m pip install -r (Join-Path $projectRoot 'services\math-ocr\requirements.txt')
if ($LASTEXITCODE -ne 0) { throw 'OCR dependency installation failed.' }
Write-Host 'Keep this console running. After Ready appears, set PAPERDESK_PADDLE_URL=http://127.0.0.1:8850/layout-parsing before starting Paperdesk.'
& $venvPython (Join-Path $projectRoot 'services\math-ocr\server.py')
