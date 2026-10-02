import "./style.css";
import { invoke } from "@tauri-apps/api/core";

type State = "passive" | "listening" | "working";
const root = document.querySelector<HTMLDivElement>("#root")!;
let state: State = "passive";
let recognition: SpeechRecognition | null = null;
let commandMode = false;

const WAKE_PHRASES = ["ehi agente", "hey agente", "ehi assistant"];

function render(message: string) {
  root.innerHTML =
    '<main class="island ' + state + '">' +
    '<div class="orb"><span></span></div>' +
    '<div class="text"><strong>Agente</strong><small>' + escapeHtml(message) + '</small></div>' +
    '</main>';
}

function escapeHtml(value: string) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

function speak(text: string) {
  if (!("speechSynthesis" in window)) return;
  speechSynthesis.cancel();
  const u = new SpeechSynthesisUtterance(text);
  u.lang = "it-IT";
  speechSynthesis.speak(u);
}

function createRecognition() {
  const w = window as unknown as { SpeechRecognition?: typeof SpeechRecognition; webkitSpeechRecognition?: typeof SpeechRecognition };
  return w.SpeechRecognition ?? w.webkitSpeechRecognition;
}

function startWakeListener() {
  const SR = createRecognition();
  if (!SR) { render("Wake word non disponibile in questa build"); return; }
  recognition = new SR();
  recognition.lang = "it-IT";
  recognition.continuous = true;
  recognition.interimResults = false;
  recognition.onresult = (event) => {
    const transcript = Array.from(event.results).slice(event.resultIndex).map(r => r[0]?.transcript ?? "").join(" ").trim().toLowerCase();
    if (!commandMode && WAKE_PHRASES.some(p => transcript.includes(p))) {
      commandMode = true;
      state = "listening";
      render("Dimmi.");
      speak("Sì, dimmi.");
      window.setTimeout(startCommandListener, 500);
    }
  };
  recognition.onerror = () => window.setTimeout(startWakeListener, 1200);
  recognition.onend = () => { if (!commandMode) window.setTimeout(startWakeListener, 250); };
  try { recognition.start(); } catch {}
}

function startCommandListener() {
  recognition?.stop();
  const SR = createRecognition();
  if (!SR) return;
  const command = new SR();
  command.lang = "it-IT";
  command.continuous = false;
  command.interimResults = false;
  command.onresult = async (event) => {
    const text = event.results[0]?.[0]?.transcript?.trim() ?? "";
    if (!text) return finish("Non ho capito.");
    await executeCommand(text);
  };
  command.onerror = () => finish("Non ho capito.");
  try { command.start(); } catch { finish("Non ho capito."); }
}

async function executeCommand(text: string) {
  state = "working";
  render(text);
  try {
    const reply = await invoke<string>("agent_message", { message: text });
    finish(reply);
  } catch (error) {
    finish(String(error).replace(/^Error:\s*/, ""));
  }
}

function finish(message: string) {
  commandMode = false;
  state = "passive";
  render(message);
  speak(message);
  window.setTimeout(() => { render("In ascolto..."); startWakeListener(); }, 1400);
}

render("In ascolto...");
startWakeListener();
