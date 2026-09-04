# Offline Translator

Qt port of my [Android offline translator](https://github.com/DavidVentura/firefox-translator), built to run on Linux phones. It also works on Linux desktop and Windows.

It performs text and image translation completely offline using on-device models.
It also supports automatic language detection, transliteration for non-Latin scripts, and a built-in word dictionary.

<p>
  <img src="screenshots/base.png" width="30%" />
  <img src="screenshots/languages.png" width="30%" />
  <img src="screenshots/tts_settings.png" width="30%" />
</p>

## How It Works

Download language packs once, then translate without sending requests to external servers.

Language packs contain the translation models, so translation happens entirely on-device.

## Tech

- Translation models are Firefox' [translations models](https://github.com/mozilla/translations)
- OCR models are [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR)
- Automatic language detection is done via [cld2](https://github.com/CLD2Owners/cld2)
- Dictionary is based on data from Wiktionary, exported by [Kaikki](https://kaikki.org/)
  - For Japanese specifically, there's a second "word dictionary" (Mecab) for transliterating Kanji
- TTS uses [Piper](https://github.com/OHF-Voice/piper1-gpl), [Coqui](https://github.com/coqui-ai/tts), [Kokoro](https://github.com/hexgrad/kokoro), [MMS](https://huggingface.co/facebook/mms-tts), [Sherpa ONNX](https://github.com/k2-fsa/sherpa-onnx), [Mimic3](https://github.com/MycroftAI/mimic3) voices
- PDF surgery uses [mupdf](https://github.com/ArtifexSoftware/mupdf) and [lopdf](https://github.com/J-F-Liu/lopdf)
- Inference engines are:
  - Translation models: [slimt](https://github.com/jerinphilip/slimt)
  - Paddle OCR, TTS, document alignment: [MNN](https://github.com/alibaba/MNN/)

## Building

Packaging notes and platform-specific build instructions live in [packaging/README.md](packaging/README.md).
