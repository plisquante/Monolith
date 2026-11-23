# Windows'ta oyunu açar, belirli süre çalıştırır ve açılışı denetler (tools/ci_game_boot.sh ile aynı kontroller):
#   - süreç hâlâ yaşıyor (çökmedi, kendiliğinden çıkmadı)
#   - çökme raporu yazılmadı
#   - günlükte "Sound engine ERROR" ve "FFI types not initialized" yok
#   - ekran görüntüsü boş değil (başlık ekranı çizildi)
# Hepsi geçerse PASS yazar ve 0 ile çıkar; aksi halde FAIL yazar ve 1 ile çıkar.
#
#   powershell -ExecutionPolicy Bypass -File tools/ci_game_boot.ps1 -Build build\minecraft-26.3 -Seconds 120
param(
    [Parameter(Mandatory = $true)][string]$Build,
    [int]$Seconds = 120,
    [string]$Out = ""
)
$ErrorActionPreference = "Stop"
$failed = $false
function Report([string]$msg) { Write-Output "FAIL: $msg"; $script:failed = $true }

if ($Out -eq "") { $Out = Join-Path $Build "boot" }
$game = Join-Path $Build "game"
New-Item -ItemType Directory -Force -Path $Out, $game | Out-Null

$exe = Join-Path $Build "rust\prog.exe"
if (-not (Test-Path $exe)) { Write-Output "FAIL: $exe not found"; exit 1 }
$crash = Join-Path $game "crash-reports"
if (Test-Path $crash) { Remove-Item -Recurse -Force $crash }

$env:MONO_RESOURCES = Join-Path $Build "resources"
$trust = Join-Path $Build "resources\cacerts"
$argList = @(
    "-Djavax.net.ssl.trustStore=`"$trust`"", "-Djavax.net.ssl.trustStoreType=JKS",
    "-Dmono.graphics.backend=opengl", "--version", "26.3", "--gameDir", "`"$game`"",
    "--assetsDir", "`"$(Join-Path $Build 'assets')`"", "--assetIndex", "34",
    "--username", "Player", "--accessToken", "0", "--userType", "legacy"
) -join " "

$stdout = Join-Path $Out "game.out.log"
$stderr = Join-Path $Out "game.log"
$p = Start-Process -FilePath $exe -ArgumentList $argList -PassThru -NoNewWindow `
    -RedirectStandardOutput $stdout -RedirectStandardError $stderr

Start-Sleep -Seconds $Seconds

# ekran görüntüsü: birincil ekranın tamamı
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$shot = Join-Path $Out "title.png"
$bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$gfx = [System.Drawing.Graphics]::FromImage($bmp)
$gfx.CopyFromScreen($bounds.X, $bounds.Y, 0, 0, $bmp.Size)
$bmp.Save($shot, [System.Drawing.Imaging.ImageFormat]::Png)

# parlaklığın standart sapması (0..1) bir ızgarada: çizim yoksa ~0
$vals = @()
for ($i = 0; $i -lt 32; $i++) {
    for ($j = 0; $j -lt 18; $j++) {
        $c = $bmp.GetPixel([int]($bmp.Width * ($i + 0.5) / 32), [int]($bmp.Height * ($j + 0.5) / 18))
        $vals += (0.299 * $c.R + 0.587 * $c.G + 0.114 * $c.B) / 255.0
    }
}
$mean = ($vals | Measure-Object -Average).Average
$sd = [Math]::Sqrt((($vals | ForEach-Object { ($_ - $mean) * ($_ - $mean) }) | Measure-Object -Sum).Sum / $vals.Count)
$gfx.Dispose(); $bmp.Dispose()
Write-Output ("screenshot variation: {0:N4}" -f $sd)

if ($p.HasExited) { Report "the game exited before the end of the $Seconds s run (see $stderr)" }
if (Test-Path $crash) {
    $reports = Get-ChildItem $crash -ErrorAction SilentlyContinue
    if ($reports) { Report "crash report written: $($reports[0].Name)" }
}
$logs = @($stdout, $stderr) | Where-Object { Test-Path $_ }
if ($logs | Where-Object { Select-String -Path $_ -Pattern "Sound engine ERROR" -Quiet }) { Report "the sound engine reported an error" }
if ($logs | Where-Object { Select-String -Path $_ -Pattern "FFI types not initialized" -Quiet }) { Report "FFI types were not initialized (JNA emulation)" }
if ($sd -lt 0.01) { Report "the screenshot is blank (nothing was drawn)" }

if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force }

if (-not $failed) {
    Write-Output "PASS: the game ran $Seconds s; screenshot: $shot"
    exit 0
}
Write-Output "--- last lines of the game log:"
if (Test-Path $stderr) { Get-Content $stderr -Tail 40 }
exit 1
