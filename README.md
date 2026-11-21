# HiResAudio Upload

Ứng dụng desktop (Tauri 2, Windows) để admin tải MP3 lên Cloudflare R2 cho app Android HiResAudio.
Mỗi file được xử lý theo logic core (không encode lại audio): probe, dedupe theo SHA-256, đọc LAME gapless,
đo loudness/true peak/LRA, waveform 800 điểm, tách ảnh bìa, remux khi ảnh bìa quá lớn. Sau đó ghi
`audio/`, `cover/`, `meta/` và `catalog.json` lên bucket. Có thể sửa tag, đổi ảnh bìa và xoá nhiều bài.

## Yêu cầu

- Rust (stable, edition 2024) và Node.js (có npm)
- WebView2 runtime (có sẵn trên Windows 11)
- ffmpeg: **chỉ** cần khi tạo lại fixtures cho test (và test perf), không cần để chạy app

## Thiết lập Cloudflare R2

1. Tạo bucket R2.
2. Bật public access cho bucket: **Settings > Public access**, bật domain `r2.dev` hoặc gắn custom domain.
   Ghi lại URL công khai (ví dụ `https://pub-xxxx.r2.dev`).
3. Tạo API token: **R2 > Manage API tokens > Create API token**, quyền **Object Read & Write**,
   giới hạn (scope) vào đúng bucket này. Lưu Access Key ID và Secret Access Key.
4. Mở app, vào **Settings** và nhập:

| Trường | Giá trị |
|---|---|
| Account ID | Cloudflare account ID (có trên trang R2 overview) |
| Access Key ID | từ API token |
| Secret Access Key | từ API token |
| Bucket | tên bucket |
| Public base URL | URL công khai của bucket, không có `/` ở cuối |

Settings được lưu ở `%APPDATA%\audios.hi-res.upload\settings.json`. Bấm kiểm tra kết nối trước khi upload.

## Chạy và build

```
npm install
npm run tauri dev      # chạy ở chế độ dev
npm run tauri build    # tạo installer trong src-tauri/target/release/bundle/
```

## Cấu trúc R2 và catalog (hợp đồng cho app Android)

| Key | Content-Type | Cache-Control |
|---|---|---|
| `audio/<id>.mp3` (`id` = SHA-256 của file phục vụ) | `audio/mpeg` | `public, max-age=31536000, immutable` |
| `cover/<coverSha256>.<jpg\|png>` | `image/jpeg` / `image/png` | `public, max-age=31536000, immutable` |
| `meta/<id>.json` (bản ghi đầy đủ, có waveform) | `application/json` | `no-cache` |
| `catalog.json` | `application/json` | `no-cache` |

`catalog.json` = `{ version: 1, updatedAt, tracks: [TrackSummary] }`. TrackSummary có mọi trường
của bản ghi đầy đủ **trừ `waveform`** (waveform chỉ nằm trong `meta/<id>.json`). Cover được đặt key theo hash
của chính ảnh, nên đổi ảnh bìa sẽ ra URL mới. Audio không bao giờ bị sửa sau khi upload.

- Thiết kế đầy đủ: [docs/superpowers/specs/2026-11-21-hiresaudio-upload-design.md](docs/superpowers/specs/2026-11-21-hiresaudio-upload-design.md)
- Logic xử lý MP3 (gapless, loudness, remux...): [docs/logic-core.md](docs/logic-core.md)

## Chạy test

```
cd src-tauri && cargo test      # unit/integration test (không cần mạng)
npm test                        # test frontend (vitest)
```

Test live (cần R2 thật, mặc định bị `#[ignore]`). Dùng biến môi trường, hoặc `e2e_live` tự đọc
settings của app nếu không có biến môi trường:

```
cd src-tauri
export R2_ACCOUNT_ID=... R2_ACCESS_KEY_ID=... R2_SECRET_ACCESS_KEY=... R2_BUCKET=... R2_PUBLIC_BASE_URL=...
cargo test --test r2_live -- --ignored --nocapture     # kết nối + put/get/delete
cargo test --test e2e_live -- --ignored --nocapture    # upload, sửa, xoá qua public URL
```

`e2e_live` chỉ sửa và xoá những bài do chính nó thêm vào. Không dùng app trên cùng bucket trong lúc `e2e_live` chạy (catalog được ghi đè theo kiểu read-modify-write).

Test hiệu năng (4 phút, 320k stereo, phải xử lý < 4 s; cần ffmpeg):

```
cd src-tauri
FFMPEG=/path/to/ffmpeg cargo test --release --test pipeline perf_4min -- --ignored --nocapture
```

## Tạo lại fixtures

```
FFMPEG=/path/to/ffmpeg bash scripts/gen-fixtures.sh
```

Chạy trong Git Bash ở thư mục gốc. Script ghi lại `src-tauri/tests/fixtures/*.mp3` và `oracle.json`
(giá trị loudness từ ffmpeg ebur128).
