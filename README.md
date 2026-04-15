# Anamedi Local

Offline speech-to-text for healthcare professionals. Press a shortcut, speak, and have your words transcribed locally. Runs on macOS, Windows, and Linux. Optional Anamedi Cloud features for doctors. Open source (MIT).

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

**Privacy policy:** The app runs fully offline by default. Doctors may optionally use Anamedi Cloud; when enabled, data is transferred to Anamedi’s cloud services. Cloud features are opt-in and can be disabled at any time.

## License

MIT License - see [LICENSE](LICENSE) for details.
