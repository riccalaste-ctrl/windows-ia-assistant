$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Speech

$recognizers = [System.Speech.Recognition.SpeechRecognitionEngine]::InstalledRecognizers()
if ($recognizers.Count -eq 0) {
  Write-Output "ERROR|Nessun riconoscitore vocale Windows installato."
  exit 2
}

$info = $recognizers | Where-Object { $_.Culture.Name -eq "it-IT" } | Select-Object -First 1
if ($null -eq $info) {
  $info = $recognizers | Where-Object { $_.Culture.TwoLetterISOLanguageName -eq "it" } | Select-Object -First 1
}
if ($null -eq $info) {
  $info = $recognizers | Select-Object -First 1
}

$engine = New-Object System.Speech.Recognition.SpeechRecognitionEngine($info.Id)
$wakeChoices = New-Object System.Speech.Recognition.Choices
[void]$wakeChoices.Add("ehi agente")
[void]$wakeChoices.Add("hey agente")
[void]$wakeChoices.Add("ehi assistant")
$wakeBuilder = New-Object System.Speech.Recognition.GrammarBuilder($wakeChoices)
$wakeGrammar = New-Object System.Speech.Recognition.Grammar($wakeBuilder)
$wakeGrammar.Name = "WakeWord"
$dictationGrammar = New-Object System.Speech.Recognition.DictationGrammar
$dictationGrammar.Name = "Command"
$dictationGrammar.Enabled = $false

$engine.LoadGrammar($wakeGrammar)
$engine.LoadGrammar($dictationGrammar)
$engine.SetInputToDefaultAudioDevice()

$script:mode = "wake"
$engine.add_SpeechRecognized({
  param($sender, $event)
  if ($event.Result.Confidence -lt 0.55) { return }

  if ($script:mode -eq "wake") {
    $wakeGrammar.Enabled = $false
    $dictationGrammar.Enabled = $true
    $script:mode = "command"
    Write-Output "WAKE"
    [Console]::Out.Flush()
  } else {
    $text = $event.Result.Text.Trim()
    if ($text.Length -gt 0) {
      $dictationGrammar.Enabled = $false
      $wakeGrammar.Enabled = $true
      $script:mode = "wake"
      Write-Output ("COMMAND|" + $text)
      [Console]::Out.Flush()
    }
  }
})

Write-Output "READY|$($info.Culture.Name)|$($info.Name)"
[Console]::Out.Flush()
$engine.RecognizeAsync([System.Speech.Recognition.RecognizeMode]::Multiple)

while ($true) {
  Start-Sleep -Seconds 1
}
