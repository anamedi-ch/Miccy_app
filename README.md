# Miccy

Offline speech-to-text for healthcare professionals. Press a shortcut, speak, and have your words transcribed locally. Runs on macOS, Windows, and Linux. Open source (MIT).

## What Miccy does

Miccy is a desktop app built with [Tauri](https://tauri.app/). It captures audio from your microphone, runs speech recognition on your machine (no cloud required for transcription), and can paste or copy the result into your workflow. Optional post-processing uses a local model when you enable it.

## Install

### Pre-built binaries

Download installers for your platform from **GitHub Releases**:

[https://github.com/anamedi-ch/anamedi_lokal/releases/latest](https://github.com/anamedi-ch/anamedi_lokal/releases/latest)

Choose the asset that matches your OS (for example `.dmg` on macOS, `.msi` or `.exe` on Windows, `.deb`, `.rpm`, or `.AppImage` on Linux).

### Build from source

For prerequisites, platform packages, and development commands, see **[BUILD.md](BUILD.md)**. In short:

```bash
git clone git@github.com:anamedi-ch/anamedi_lokal.git
cd anamedi_lokal
bun install
mkdir -p src-tauri/resources/models
curl -o src-tauri/resources/models/silero_vad_v4.onnx https://blob.handy.computer/silero_vad_v4.onnx
bun run tauri dev
```

On macOS, if you hit CMake policy errors, try: `CMAKE_POLICY_VERSION_MINIMUM=3.5 bun run tauri dev`.

## Contributing

This project improves fastest with **active contributions**: bug reports with clear reproduction steps, documentation fixes, translations, and focused pull requests are all welcome.

- Read **[CONTRIBUTING.md](CONTRIBUTING.md)** for workflow, style, and how to propose changes.
- Use [Issues](https://github.com/anamedi-ch/anamedi_lokal/issues) for defects and [Discussions](https://github.com/anamedi-ch/anamedi_lokal/discussions) for ideas and questions.
- If you are new to the codebase, look for issues labeled `good first issue` or `help wanted` when they appear.

## Languages

Coverage falls into three layers (see `src/i18n/languages.ts`, `src/lib/constants/languages.ts`, and post-processing prompts in `src-tauri/src/settings.rs`):

| Layer | What’s covered |
|--------|----------------|
| **App UI** | **16** interface locales: English, Chinese, Spanish, French, German, Japanese, Korean, Vietnamese, Polish, Italian, Russian, Ukrainian, Portuguese, Czech, Turkish, Arabic (RTL). |
| **Speech recognition** | **Auto** language detection, or choose from **50+** Whisper-oriented language codes (including German, French, Italian, English, Swiss national languages, and many others). Behaviour can differ slightly by transcription engine (e.g. Parakeet auto-detect). Optional **translate to English** for Whisper where supported. |
| **Note templates / local LLM** | Prompts are **multilingual in principle**; built-in clinical examples skew toward **German** and can be edited per deployment. |

## Code signing policy

Free code signing provided by SignPath.io, certificate by SignPath Foundation.

- **Authors:** [jempf](https://github.com/jempf)
- **Reviewers:** [jempf](https://github.com/jempf)
- **Approvers:** [jempf](https://github.com/jempf)

**Privacy policy:** Speech recognition and optional post-processing are designed to run on your machine; model downloads and app updates use the network as needed.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Upstream

Miccy is a **fork** of **[Handy](https://github.com/cjpais/Handy)** by **CJPais**. The original project did the heavy lifting: architecture, offline-first design, and much of the core implementation. I am very grateful for that work and encourage anyone interested in the lineage of the code to explore the upstream repository.
