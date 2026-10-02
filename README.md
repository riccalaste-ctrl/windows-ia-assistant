# Windows IA Assistant

Personal Windows voice agent built from the Windows/Tauri foundation of Coucou, reworked for a cloud-first assistant rather than Claude Code session monitoring.

## Architecture

- Local Windows process stays resident in passive mode.
- A local wake-word/audio adapter is the only component that should listen continuously.
- After activation, cloud AI handles reasoning and tool selection.
- Rust executes a permissioned local tool set; the model never receives arbitrary shell access.
- OpenAI credentials are stored in Windows Credential Manager. OPENAI_API_KEY is accepted only as a development fallback.
- Read/open/search actions can run automatically; file mutations require an explicit confirmation in the island UI.
- Agent continuity is kept through the Responses API response chain while the app is running.
- Single-instance and per-user autostart support are included.

## Voice boundary

The previous browser SpeechRecognition implementation has been removed. It was not a true local wake-word detector and could not satisfy the privacy boundary of the intended architecture.

The remaining integration point is deliberately local: the wake-word/audio adapter should call the same agent command after detecting the activation phrase. This keeps the cloud path inactive until activation.

## Coucou attribution

The Windows/Tauri application foundation was derived from the MIT-licensed Windows portion of Coucou by Louis Raillé. This project does not ship Coucou's Mochi name, character, sounds or media.

Source foundation: https://github.com/Louis-CFM/coucou
