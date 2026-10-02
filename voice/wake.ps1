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

$engine = [System.Speech.Recognition.SpeechRecognitionEngine]::new($info.Id)
$wakeChoices = [System.Speech.Recognition.Choices]::new()
[void]$wakeChoices.Add("ehi agente")
[void]$wakeChoices.Add("hey agente")
[void]$wakeChoices.Add("ehi assistant")
$wakeBuilder = [System.Speech.Recognition.GrammarBuilder]::new($wakeChoices)
$wakeGrammar = [System.Speech.Recognition.Grammar]::new($wakeBuilder)
$wakeGrammar.Name = "WakeWord"
$dictationGrammar = [System.Speech.Recognition.DictationGrammar]::new()
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
