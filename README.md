# Forge Queue

![Forge Queue illustrated cover](web/cover.svg)

**English** · [Русский](#русский)

A local image workshop: resize a batch, follow its progress and download the results as a ZIP. Built with Rust.

## Start

Install [Rust and Cargo](https://rustup.rs/), run `cargo run --release` and open <http://127.0.0.1:3003>. Windows: `run.bat`; macOS: `sh run.command`. Stop with Ctrl+C.

Choose up to 10 JPEG, PNG or WebP images, each up to 5 MiB and 16 million pixels. Set maximum width and height (1–4000 pixels), choose JPEG, PNG or WebP output and start the batch. Images are resized to fit while preserving aspect ratio. The ZIP contains generated filenames; original files are untouched. Transparent pixels are discarded in JPEG output. Encoding uses library defaults; quality is not adjustable yet.

One batch runs at a time, processing one image at a time. A damaged image is marked as failed; other images can still complete. Uploads, queue and ZIP are kept in process memory and cleared on restart or when **Clear queue** is used. No images are uploaded to an external service. The local server is not intended to be exposed on a network. The cover is an illustration, not a screenshot.

## Русский

Локальная мастерская: измените размеры нескольких изображений, следите за обработкой и скачайте результат архивом ZIP. Основная логика написана на Rust.

Установите [Rust и Cargo](https://rustup.rs/), запустите `cargo run --release` и откройте <http://127.0.0.1:3003>. Windows: `run.bat`; macOS: `sh run.command`. Остановка — Ctrl+C.

Выберите до 10 файлов JPEG, PNG или WebP: каждый до 5 МиБ и 16 миллионов пикселей. Укажите максимальную ширину и высоту от 1 до 4000 пикселей, формат и начните обработку. Изображение вписывается в размеры с сохранением пропорций. В архиве создаются новые имена; оригиналы не изменяются. При сохранении в JPEG прозрачность теряется. Настройка качества пока недоступна.

Одновременно выполняется одна партия; файлы обрабатываются по одному. Повреждённый файл отмечается ошибкой, остальные продолжают обрабатываться. Файлы, очередь и ZIP хранятся в памяти приложения и удаляются при остановке или нажатии **Очистить очередь**. Изображения не отправляются во внешние сервисы. Сервер предназначен для локального использования. Обложка — иллюстрация, не скриншот.
