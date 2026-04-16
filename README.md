# Miccy

Offline speech-to-text for healthcare professionals. Press a shortcut, speak, and have your words transcribed locally. Runs on macOS, Windows, and Linux. Open source (MIT).

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
