# Forge Queue

![Forge Queue illustrated cover](web/cover.svg)

**English** · [Русский](#русский)

A local image workshop: resize a batch, follow its progress and download the results as a ZIP. Built with Rust.

## Start

Install [Rust and Cargo](https://rustup.rs/), run `cargo run --release` and open <http://127.0.0.1:3003>. Windows: `run.bat`; macOS: `sh run.command`. Stop with Ctrl+C.

Choose up to 10 JPEG, PNG or WebP images, each up to 5 MiB and 16 million pixels. Set maximum width and height (1–4000 pixels), choose JPEG, PNG or WebP output and start the batch. Images are resized to fit while preserving aspect ratio. The ZIP contains generated filenames; original files are untouched. Transparent pixels are discarded in JPEG output. JPEG quality can be set from 1 to 100; PNG and WebP use library defaults. File sizes and percentage change appear after processing.

One batch runs at a time, processing one image at a time. A damaged image is marked as failed; other images can still complete. Uploads, queue and ZIP are kept in process memory and cleared on restart or when **Clear queue** is used. No images are uploaded to an external service. The local server is not intended to be exposed on a network. The cover is an illustration, not a screenshot.

## Русский

Локальная мастерская: измените размеры нескольких изображений, следите за обработкой и скачайте результат архивом ZIP. Основная логика написана на Rust.

Установите [Rust и Cargo](https://rustup.rs/), запустите `cargo run --release` и откройте <http://127.0.0.1:3003>. Windows: `run.bat`; macOS: `sh run.command`. Остановка — Ctrl+C.

Выберите до 10 файлов JPEG, PNG или WebP: каждый до 5 МиБ и 16 миллионов пикселей. Укажите максимальную ширину и высоту от 1 до 4000 пикселей, формат и начните обработку. Изображение вписывается в размеры с сохранением пропорций. В архиве создаются новые имена; оригиналы не изменяются. При сохранении в JPEG прозрачность теряется. Качество JPEG можно задать от 1 до 100; PNG и WebP используют настройки библиотеки. После обработки показаны размеры файлов и изменение в процентах.

Одновременно выполняется одна партия; файлы обрабатываются по одному. Повреждённый файл отмечается ошибкой, остальные продолжают обрабатываться. Файлы, очередь и ZIP хранятся в памяти приложения и удаляются при остановке или нажатии **Очистить очередь**. Изображения не отправляются во внешние сервисы. Сервер предназначен для локального использования. Обложка — иллюстрация, не скриншот.

### Orientation / Ориентация
EXIF orientation (including mirrored orientations) is applied once before resizing. Output is newly encoded without copying EXIF/GPS metadata. Damaged orientation metadata can fail that item; others continue. Upload admission is released if its handler is cancelled, and multipart reception is limited to 30 seconds.
Ориентация EXIF, включая отражение, применяется перед изменением размеров. Метаданные EXIF/GPS не копируются в результат. Ошибка одного файла не останавливает остальные. Приём партии ограничен 30 секундами.
Run `cargo run --release -- --check` to check the local port without processing files. / Проверка локального порта без обработки: `--check`.


## Multipart recovery / Восстановление приёма

The existing Admission guard releases a failed/cancelled receive. New real TCP tests cover disconnect before a field, mid-file and the 30-second receive timeout. Files are now accumulated in bounded chunks (5 MiB each); known settings stop at 64 bytes. Prior items/ZIP survive an unsuccessful replacement upload; only a valid parsed replacement clears them. One batch runs at a time, up to ten files, with the existing 52 MiB request and 16 MP decode limits. These are input-work limits, not a measured total process-RSS ceiling.

A browser submission locks a second start until the user reviews/downloads the queue and explicitly clears it. The lock persists in sessionStorage when available; it does not imply durable server receipts. API POST after a completed job is a new replacement command, not an idempotent replay. Never automatically retry an unknown upload; inspect GET /api/job first. Restart still loses all in-memory jobs. No disk persistence or hardware power-loss guarantee was added.

При ошибке приёма предыдущий ZIP сохраняется. Повторное нажатие запуска заблокировано до явной очистки; сначала проверьте очередь и скачайте доступный результат. Новый API POST после завершения означает новую партию, не глобальную дедупликацию.


### Extended recovery acceptance — 14 November

Real TCP CI checks now repeat 32 aborted replacements, verify bounded open file descriptors on Linux and compare the prior ZIP byte-for-byte. Killing the process during reception leaves an honestly empty in-memory queue on restart, then a new batch succeeds. GET /api/job exposes only a fixed last receive_error code (timeout/limit/invalid/interrupted), cleared on new admission or explicit Clear. EN/RU UI explains recovery without copying incoming multipart names into diagnostics. This is bounded last-state metadata, not an expanding event log. No persistence, heap/load benchmark, worker cancellation or hardware power-loss durability is promised.
