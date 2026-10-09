# Aquilum

## Development

```powershell
npm install
npm run tauri dev
```

## Verification

```powershell
npm test
cd src-tauri
cargo test
```

## Release

```powershell
npm run release
```

Финальные файлы: `../.artifacts/releases/<version>/` (отсюда запускай `.exe`).

Кэши разделены:
- `../.artifacts/cargo-dev/` — `npm run tauri dev`
- `../.artifacts/cargo-release/` — фабрика `npm run release` (не для ручного запуска)
- `../.artifacts/dist/` — production-фронт внутри сборки

Подробности: `../knowledge base/build-artifacts-separation.md`.
