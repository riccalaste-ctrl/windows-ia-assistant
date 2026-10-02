import "./style.css";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type State = "passive" | "listening" | "working" | "confirming";

const root = document.querySelector<HTMLDivElement>("#root")!;
let state: State = "passive";
let message = "In ascolto…";
let pendingConfirmation = false;
let wakeActive = false;

function escapeHtml(value: string) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function render() {
  root.innerHTML = `
    <main class="island ${state}">
      <div class="orb" aria-hidden="true"><span></span><i></i></div>
      <div class="copy"><strong>Agente</strong><small>${escapeHtml(message)}</small></div>
      ${pendingConfirmation ? '<div class="actions"><button id="deny">Annulla</button><button id="allow">Conferma</button></div>' : ""}
    </main>`;
  root.querySelector<HTMLButtonElement>("#allow")?.addEventListener("click", () => void confirm(true));
  root.querySelector<HTMLButtonElement>("#deny")?.addEventListener("click", () => void confirm(false));
}

function speak(text: string) {
  if (!("speechSynthesis" in window)) return;
  window.speechSynthesis.cancel();
  const utterance = new SpeechSynthesisUtterance(text);
  utterance.lang = "it-IT";
  utterance.rate = 1.04;
  window.speechSynthesis.speak(utterance);
}

async function confirm(confirmed: boolean) {
  state = "working";
  pendingConfirmation = false;
  message = confirmed ? "Eseguo…" : "Annullato.";
  render();

  try {
    message = await invoke<string>("confirm_pending", { confirmed });
  } catch (error) {
    message = String(error).replace(/^Error:\s*/, "");
  }

  state = "passive";
  render();
  speak(message);
}

async function sendCommand(text: string) {
  state = "working";
  message = text;
  render();

  try {
    const reply = await invoke<string>("agent_message", { message: text });
    message = reply;

    if (reply.startsWith("Devo confermare l'azione")) {
      pendingConfirmation = true;
      state = "confirming";
    } else {
      state = "passive";
      speak(reply);
    }
  } catch (error) {
    state = "passive";
    message = String(error).replace(/^Error:\s*/, "");
    speak(message);
  }

  render();
}

const fallbackInput = document.createElement("input");
fallbackInput.className = "fallback-input";
fallbackInput.placeholder = "Comando";
fallbackInput.autocomplete = "off";
fallbackInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter" && fallbackInput.value.trim()) {
    const text = fallbackInput.value.trim();
    fallbackInput.value = "";
    void sendCommand(text);
  }
});
document.body.appendChild(fallbackInput);

void listen<string>("voice-event", (event) => {
  const payload = event.payload;

  if (payload === "WAKE") {
    wakeActive = true;
    state = "listening";
    message = "Sì, dimmi.";
    render();
    speak("Sì, dimmi.");
    return;
  }

  if (payload.startsWith("COMMAND|") && wakeActive) {
    wakeActive = false;
    const command = payload.slice("COMMAND|".length).trim();
    if (command) void sendCommand(command);
    return;
  }

  if (payload.startsWith("READY|")) {
    state = "passive";
    message = "In ascolto…";
    render();
    return;
  }

  if (payload.startsWith("ERROR|")) {
    state = "passive";
    message = payload.slice("ERROR|".length);
    render();
  }
});

render();
