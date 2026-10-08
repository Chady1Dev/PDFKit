param(
    [Parameter(Mandatory=$true)]
    [string]$ImagePath,
    [string]$Language
)

$ErrorActionPreference = "Stop"

try {
    Add-Type -AssemblyName System.Runtime.WindowsRuntime

    $asTaskGeneric = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
        $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
    } | Select-Object -First 1

    function Await($asyncOp, $type) {
        $asTask = $asTaskGeneric.MakeGenericMethod($type)
        $task = $asTask.Invoke($null, @($asyncOp))
        $task.Wait()
        return $task.Result
    }

    [Windows.Storage.StorageFile, Windows.Storage, ContentType = WindowsRuntime] | Out-Null
    [Windows.Graphics.Imaging.BitmapDecoder, Windows.Graphics.Imaging, ContentType = WindowsRuntime] | Out-Null
    [Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType = WindowsRuntime] | Out-Null

    $resolvedPath = [System.IO.Path]::GetFullPath($ImagePath)
    if (-not (Test-Path -Path $resolvedPath)) {
        throw "Image file not found: $resolvedPath"
    }

    $engine = $null
    if ($Language) {
        $langObj = New-Object Windows.Globalization.Language $Language
        if ([Windows.Media.Ocr.OcrEngine]::IsLanguageSupported($langObj)) {
            $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage($langObj)
        }
    }
    if ($null -eq $engine) {
        $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
    }
    if ($null -eq $engine) {
        $available = [Windows.Media.Ocr.OcrEngine]::AvailableRecognizerLanguages
        if ($available.Count -gt 0) {
            $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage($available[0])
        }
    }
    if ($null -eq $engine) {
        throw "No OCR engine could be initialized on this system."
    }

    $file = Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync($resolvedPath)) ([Windows.Storage.StorageFile])
    $stream = Await ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
    $decoder = Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
    $bitmap = Await ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
    $ocrResult = Await ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])

    $wordsList = [System.Collections.Generic.List[object]]::new()
    foreach ($line in $ocrResult.Lines) {
        foreach ($w in $line.Words) {
            $wordsList.Add(@{
                text = $w.Text
                x = [double]$w.BoundingRect.X
                y = [double]$w.BoundingRect.Y
                width = [double]$w.BoundingRect.Width
                height = [double]$w.BoundingRect.Height
            })
        }
    }

    $outputObj = @{
        text = $ocrResult.Text
        words = $wordsList
        image_width = [double]$decoder.PixelWidth
        image_height = [double]$decoder.PixelHeight
    }

    $json = $outputObj | ConvertTo-Json -Depth 5 -Compress
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
    [Console]::WriteLine($json)
} catch {
    $errObj = @{
        error = $_.Exception.Message
    }
    $json = $errObj | ConvertTo-Json -Compress
    [Console]::Error.WriteLine($json)
    exit 1
}
