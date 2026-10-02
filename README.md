# Windows IA Assistant

Windows IA Assistant is a Windows-first personal voice agent built from the Windows/Tauri interaction foundation of Coucou, reworked so it is not a Claude Code companion.

Say "Ehi agente" and the assistant wakes, speaks with you, executes requested Windows/browser/file actions through local tools, then returns to passive listening.

Architecture:
- Cloud AI: reasoning, conversation, tool selection and planning through an API provider.
- Local Windows bridge: executes approved actions on Windows.
- Local wake-word layer: listens only for the activation phrase while idle.
- Tauri 2 + TypeScript: lightweight always-available Windows UI.

API credentials belong in Windows Credential Manager, never in frontend source code.

The Windows UI/runtime foundation is derived from the Windows portion of Coucou: https://github.com/Louis-CFM/coucou
