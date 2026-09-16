# RF-Organ working conventions

- Keep source code, documentation, metadata, tests and CLI messages in English. Speak to the user in their preferred language.
- The project is GPL-2.0-or-later. Preserve third-party attribution and record translated algorithms or data tables in `THIRD_PARTY_NOTICES.md`.
- Production DSP is written in Rust. Do not embed or compile C/C++ implementations from reference projects into the plugin.
- The Leslie is a user-facing module inside RF-Organ. It must be addressable by parameters, MIDI controls, saved state and programs. Keep its DSP internally isolated and testable.
- Treat physical plausibility and empirical calibration as different milestones. Clearly label provisional constants and never present an unvalidated approximation as a measured model.
- The audio callback must not allocate, log, access files, acquire locks or perform unbounded work.
- Keep local build growth bounded: use `CARGO_INCREMENTAL=0` for validation builds and do not commit generated packages or build artifacts.

