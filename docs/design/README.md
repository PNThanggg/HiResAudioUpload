# HiResAudio Upload: UI design

Design pass for Task 11, made with the `ui-ux-pro-max` skills (`design` → `ui-ux-pro-max` + `design-system`).
Files:

| File | What |
|---|---|
| `src/styles/tokens.css` | All design tokens (CSS custom properties), light + dark |
| `docs/design/mockup.html` | Static mockup: 4 app frames at 1280 × 800 + a component/state sheet. Its `<style>` block is the component CSS: lift everything above the `MOCKUP-ONLY` marker into `src/styles/app.css` |
| `docs/design/README.md` | This file: direction, tokens, component specs, copy |

Open `mockup.html` straight from disk (it links `../../src/styles/tokens.css`). The buttons at the top right switch between system / light / dark.

## 1. Direction: "Mastering Console"

A quiet graphite workspace with a single **signal-violet** accent. The audio-quality colors behave like **meter LEDs**: green / cyan / amber / red, each paired with a 4-step bar glyph and a text label. Numbers are tabular and right-aligned, the way they are on a mastering desk.

Why this direction:
- The skill's product data pairs *Music Streaming* and *Developer Tool* with "Dark Mode + Minimalism", and *Data-Dense Dashboard* with tables, sticky headers, 8–12 px padding and row highlighting. This is a hybrid: a dense data tool for audio files.
- The skill's Music palettes (`#1E1B4B` indigo + play-green, studio purple + waveform green) point to violet as the brand hue. Violet also stays clear of the four quality colors, so the accent never reads as a quality signal. A trust-blue accent would collide with the cyan "High" level and look like generic SaaS (spec anti-goal: "a generic-looking admin UI").
- Light and dark are equal citizens. The app follows the OS setting, and `<html data-theme="light|dark">` forces one.

What makes it not generic: the quality ladder works like a meter (bars + color + words), the bandwidth meter shows *where the content actually stops* against the LC §5.2 thresholds, the waveform sits as a strip in the detail panel, true peak above 0 dBTP gets a warning, and all technical numbers use tabular figures.

## 2. Tokens (`src/styles/tokens.css`)

Three layers, following the `design-system` skill:
1. **Primitives** `--p-*` (gray, violet, green, cyan, amber, red ramps). Only `tokens.css` uses them.
2. **Semantic** `--color-*`, `--space-*`, `--radius-*`, `--font-*`, `--size-*`, `--duration-*`, `--ease-*`, `--z-*`, `--shadow-*`. Components use only these.
3. **Component** tokens live inside component CSS where they help, e.g. `.qbadge { --q-fg / --q-soft / --q-border }`, `.toast { --toast-accent }`.

Theme switching: light values sit on `:root`. Dark values are in `@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) }` and repeated in `:root[data-theme="dark"]`. **Keep those two dark blocks identical.**

### Color (semantic)

| Token | Light | Dark | Use |
|---|---|---|---|
| `--color-bg` | `#f4f5f7` | `#0d0f12` | Window background |
| `--color-surface` | `#ffffff` | `#16181d` | Table card, panels, topbar, inputs |
| `--color-surface-sunken` | `#f4f5f7` | `#121418` | Table header, waveform well, stat tiles |
| `--color-surface-raised` | `#ffffff` | `#1d2026` | Dialogs, toasts |
| `--color-hover` / `--color-pressed` | gray-75 / gray-100 | gray-850 / gray-800 | Ghost/secondary button states |
| `--color-border` / `-strong` | gray-100 / gray-200 | gray-800 / gray-750 | Dividers / button outlines |
| `--color-border-input` | `#878e9b` | `#646b77` | Input and drop-zone borders (≥ 3:1, WCAG 1.4.11) |
| `--color-text` | `#16181d` | `#e9ebef` | Primary text |
| `--color-text-muted` | `#535a66` | `#a9afba` | Secondary text, artist, labels |
| `--color-text-subtle` | `#6a717e` | `#8c93a0` | Column headers, hints, ticks (still ≥ 4.5:1) |
| `--color-accent` | `#5b43e6` | `#6650ee` | Primary button, checkbox, active row bar, nav underline |
| `--color-accent-hover` | `#4a32cc` | `#5b43e6` | |
| `--color-accent-text` | `#5b43e6` | `#a99cff` | Links, accent text on surfaces |
| `--color-accent-soft` / `-border` | violet-50 / violet-300 | violet-950 / violet-700 | Selection bar, drag-over |
| `--color-focus` | `#5b43e6` | `#a99cff` | Focus ring |
| `--color-row-hover` / `-selected` / `-active` | | | Table row states (see §4.3) |
| `--color-success` / `info` / `warning` / `danger` (+ `-soft`, `-border`) | green-700 / cyan-700 / amber-700 / red-700 | green-400 / cyan-400 / amber-400 / red-400 | Status text, tinted backgrounds |
| `--color-danger-solid` (+ `-hover`), `--color-danger-fg` | red-600 | red-600 | Destructive button |
| `--color-quality-{full,high,medium,low,none}` (+ `-soft`, `-border`) | alias success / info / warning / danger / muted | | Quality badge + bandwidth meter |
| `--color-waveform`, `--color-waveform-baseline` | violet-600 / gray-200 | violet-400 / gray-800 | Waveform |
| `--color-cover-placeholder` (+ `-icon`) | | | Missing cover |
| `--color-skeleton`, `--color-scrim` | | | Loading, dialog backdrop |

**Contrast:** I checked every text/background pair above with a WCAG script (light and dark). All text pairs are ≥ 4.5:1. The lowest are `text-subtle` on `bg`, light, at 4.50, and white on the dark-mode accent at 5.32. Focus ring and input borders are ≥ 3:1. Do not put `--color-text-subtle` on `--color-row-selected` or the tinted `-soft` fills: use `--color-text-muted` there.

### Type

| Token | Value |
|---|---|
| `--font-sans` | `"Segoe UI Variable Text", "Segoe UI", system-ui, …` |
| `--font-display` | `"Segoe UI Variable Display", "Segoe UI", …` (titles, metric numbers) |
| `--font-mono` | `"Cascadia Mono", "Cascadia Code", Consolas, …` (IDs, keys, error codes, file keys) |
| `--font-size-xs/sm/md/lg/xl/2xl/3xl` | 12 / 13 / 14 / 16 / 18 / 22 / 26 px |
| `--font-weight-regular/medium/semibold/bold` | 400 / 500 / 600 / 700 |
| `--font-line-tight/base` | 1.2 / 1.45 |

- **No web font.** The app runs offline in WebView2, so it uses system fonts. Segoe UI ships with Windows and covers Vietnamese diacritics completely. Cascadia Mono ships with Windows Terminal on Windows 11, and Consolas is the fallback. The skill suggested Inter, Fira Sans / Fira Code, or IBM Plex + JetBrains Mono. If a custom face is ever wanted, self-host **Inter** (OFL, has Vietnamese) under `src/assets/fonts/` and prepend it to `--font-sans`. Do not use a CDN.
- Base size is 14 px, table cells 13 px, and nothing goes below 12 px. This is a dense desktop tool used with a mouse, not a mobile screen.
- **Sentence case everywhere.** No all-caps labels: stacked Vietnamese diacritics on capitals ("THỜI LƯỢNG") crowd and clip at small sizes.
- Every number column, metric, size, duration and counter gets `.num` (`font-variant-numeric: tabular-nums`).

### Space, radius, sizes, motion, layers

- `--space-*`: a 4 px grid. `0, px(1), 0-5(2), 1(4), 1-5(6), 2(8), 3(12), 4(16), 5(20), 6(24), 8(32), 10(40), 12(48)`.
- `--radius-xs/sm/md/lg/full`: 3 / 4 / 6 / 10 / 999 px. Badges and row covers use sm, buttons and inputs md, panels and dialogs lg.
- `--size-*`: topbar 52, toolbar 48, row 48 (two-line), row-compact 36, control 32, control-sm 26, icon 16 / 20, cover-row 32, cover-detail 88, inspector 384, dialog 560, focus ring 2 px.
- `--duration-fast/base/slow`: 120 / 180 / 260 ms. `--ease-out` (enter), `--ease-in` (exit), `--ease-linear` (spinners, progress). Under `prefers-reduced-motion`, durations collapse to ~0, spinners stop, the indeterminate bar becomes a static half-opacity bar, and skeletons stop pulsing.
- `--z-sticky/panel/overlay/dialog/toast`: 10 / 20 / 40 / 50 / 60.
- `--shadow-sm/md/lg`: button / toast & cover / dialog.

## 3. Layout

- Window: **1280 × 800 default, min 1024 × 640** (set `width`, `height`, `minWidth`, `minHeight` in `tauri.conf.json`).
- Shell is a grid: topbar (52 px) over main. Topbar: brand, nav tabs **Thư viện** (with track count) / **Cài đặt** (2 px accent underline on `aria-current="page"`), connection status (dot + text "Đã kết nối · bucket"), primary **Tải lên** button (shows the hover fill while the upload panel is open, `aria-expanded="true"`).
- Main = library (flex) + **inspector** side panel (384 px), which shows *either* the track detail *or* the upload panel. Closing it gives the table the full width (`.main--full`).
- Library = toolbar (title + "N bài · X MB" + Làm mới) or selection bar, then the table card (scrolls, sticky header).
- Settings page: a 600 px form card plus a 300 px "Trước khi kết nối" note card.
- At window widths below ~1180 px with the inspector open, drop the **Album** column (album still shows in detail).

## 4. Component specs

All classes below are in the mockup `<style>`.

### 4.1 Buttons `.btn`
| Variant | Fill / text | Use |
|---|---|---|
| `--primary` | accent / accent-fg, hover accent-hover | One per view: Tải lên, Lưu, Lưu thay đổi |
| `--secondary` | surface + border-strong + shadow-sm | Kiểm tra kết nối, Chọn file…, Sửa, Huỷ |
| `--ghost` | transparent, muted → text on hover | Bỏ chọn, Làm mới, close ✕ |
| `--danger` | danger-solid / white | Xoá N bài (bulk bar, confirm) |
| `--danger-ghost` | danger text, danger-soft hover | Xoá in detail header |

Sizes: default 32 px, `--sm` 26 px, `--xs` 22 px, `--icon` square (icon-only buttons need `aria-label`). Disabled: 45 % opacity, `not-allowed`. Loading: `.is-loading` + spinner icon + `aria-busy="true"` + label "Đang lưu…". Focus: the global `:focus-visible` ring (2 px `--color-focus`, offset 2).

### 4.2 Cover `.cover`
32 px in rows (radius sm), 88 px in detail (radius md + shadow-md), 72 px in the edit dialog. Use `<img>` with `object-fit: cover`, `alt=""` in rows (the title sits next to it) and `alt="Ảnh bìa <album>"` in detail. **No cover** → `.cover--empty`: placeholder fill, dashed border, music-note icon, `aria-label="Không có ảnh bìa"`. Reserve the box size so nothing shifts while images load.

### 4.3 Track table row (`table.tracks`)
Columns: checkbox 40 · cover 44 · **Tiêu đề / Nghệ sĩ** (two lines: title 14 px medium + artist 12 px muted) · Album 17 % · Thời lượng 72 (right) · Chất lượng 204 · LUFS 58 (right) · Dung lượng 84 (right). Header: 34 px, sunken, 12 px semibold subtle, sticky. Rows 48 px, 1 px dividers, overflowing text ellipsized.

| State | Visual |
|---|---|
| default | surface |
| hover | `--color-row-hover` |
| **selected** (checkbox on) | `--color-row-selected` (violet tint) + checked box; the toolbar swaps to the selection bar "✓ Đã chọn N bài · Bỏ chọn · [Xoá N bài]" |
| **active** (open in detail) | `--color-row-active` + 3 px accent bar on the left edge (inset shadow on the first cell) |
| selected + active | selected tint + accent bar |
| focus (keyboard) | focus ring on the row (`tr:focus-visible`, `outline-offset: -2px`) |

Header checkbox: unchecked / checked / `indeterminate` (some selected). Keyboard: ↑/↓ move the active row (roving `tabindex`), Space toggles the checkbox, Enter opens the detail, Delete opens the delete confirm for the selection (or the active row), Ctrl+A selects all, Esc clears the selection and then closes the panel. A click on the row (not the checkbox) makes it active.

### 4.4 Quality badge `.qbadge`
`<span class="qbadge qbadge--{full|high|medium|low|none}"><span class="qmeter" aria-hidden="true"><i class="on">…×4</i></span>LABEL</span>`

| Hint | Label (`qualityLabel(bitrate, hint)`) | Color | Bars |
|---|---|---|---|
| full | `MP3 320 · Full band` | `--color-quality-full` (green) | 4 |
| high | `MP3 245 · High (~V0–V2)` | `--color-quality-high` (cyan) | 3 |
| medium, bitrate ≥ 192 | `MP3 320 · nguồn ~128k` | `--color-quality-medium` (amber) | 2 |
| medium, bitrate < 192 | `MP3 160 · Medium` | amber | 2 |
| low | `MP3 128 · Nguồn kém` | `--color-quality-low` (red) | 1 |
| null | `MP3 256` | `--color-quality-none` (muted, neutral) | 0 |

The meaning comes from three channels: the text, the number of lit bars (heights 4/6/8/10 px, unlit bars at 25 % of `currentColor`), and the color. Sizes: default 22 px / 12 px text, `--sm` 20 px (upload queue), `--lg` 28 px / 13 px (detail). Optionally add `title` with the real bandwidth ("Băng thông thực 17.15 kHz"). The badge is a status, not a button.

### 4.5 Detail panel (inspector)
Order: hero (cover 88 + title 18 px display + artist + "album · năm · thể loại" + [Sửa] [Xoá]) → **Chất lượng** (large badge + bandwidth meter + hint line) → **Waveform** → **Độ lớn & đỉnh** (3 metric tiles) → **Gapless** (3 stat tiles) → **Kỹ thuật** (key/value list). Section labels are 12 px semibold subtle. The panel body scrolls; the head (title + ✕) stays put.

- **Bandwidth meter** `.bandwidth`: the scale runs linearly from 10 kHz to Nyquist (`sampleRate / 2`). Four zones in the quality `-border` colors end at the LC §5.2 thresholds **15.5 / 18 / 19.5 kHz** (Kém / Medium / High / Full). The fill runs from the left edge to `effectiveBandwidthHz` in `--color-quality-<hint>`, with a 2 px needle (text color) at the value. Tick labels below: `10k · 15.5k · 18k · 19.5k · Nyquist`. The value is at the top right ("17.15 kHz"), and the container has a `role="img"` + `aria-label`. For 48 kHz files the scale ends at 24k. Hint line under the meter (info icon + 12 px muted), per hint: full "Nội dung trải tới gần Nyquist: đúng chất lượng bitrate ghi.", high "Mức cắt kiểu VBR V0–V2.", medium & bitrate ≥ 192 "File ghi {bitrate} kbps nhưng nội dung dừng ở ~{kHz} kHz: nhiều khả năng là bản 128 kbps được encode lại.", medium < 192 "Mức cắt kiểu 128–160 kbps.", low "Băng thông thấp: nguồn chất lượng kém."
- **Waveform** `.waveform`: a sunken well (radius md) holding a 64 px drawing area plus a 20 px time-tick row (0:00, every minute, the end time; every 2 min if longer than 8 min). The app draws on `<canvas>`: 800 peaks (0..1), one bar per point, **mirrored** around a center baseline (`--color-waveform-baseline`, 1 px). Bar half-height = `peak × (h/2 − 2)`, minimum 0.5 px, color `--color-waveform`. Size the canvas to `clientWidth × devicePixelRatio`, read colors with `getComputedStyle`, and redraw on resize and when `prefers-color-scheme` / `data-theme` changes. Give the canvas `role="img"` + `aria-label="Waveform của <title>"`.
- **Metric tiles** `.metric`: label (12 px muted), value 26 px display semibold tabular + unit (12 px muted): Integrated `-8.1 LUFS`, True peak `+0.3 dBTP`, LRA `4.6 LU`. When **true peak > 0 dBTP**, use `.metric--warn` (warning-soft fill + border) and add a note "⚠ Vượt 0 dBTP" (icon + text).
- **Gapless** `.stats`: Bỏ đầu `trimStart`, Bỏ cuối `trimEnd`, Tổng `totalSamples`, each with the unit "mẫu". When null: one hint line "ⓘ Không có thông tin gapless".
- **Kỹ thuật** `.kv` (112 px label column, 30 px rows, dividers): Định dạng "MP3 · CBR|VBR {bitrate} kbps", Tần số mẫu "44.1 kHz", Kênh "Mono|Stereo", Thời lượng, Dung lượng, Track / Disc, Nghệ sĩ album, ID (mono, ellipsized, copy icon button "Sao chép ID"), Tạo lúc, Cập nhật.

### 4.6 Drop zone `.dropzone`
- **Idle, empty queue:** a column layout with a 40 px icon circle, "Kéo thả file MP3 vào đây", "hoặc", [Chọn file…], and the limits hint "Tối đa 200 MB · dài 1 giây đến 2 giờ". 1.5 px dashed `--color-border-input`, sunken fill, radius lg.
- **Compact** (`.dropzone--compact`, once the queue has items): one row with the icon, "Kéo thả thêm file MP3" + the limits, and [Chọn file…].
- **Drag-over** (`.is-dragover`): solid accent border, accent-soft fill, accent title "Thả để tải lên N file" / "Chỉ nhận file .mp3". Tauri drag-drop events cover the **whole window**, so when files are dragged anywhere, open the upload panel and put the zone in drag-over. Silently drop non-`.mp3` paths and say so in an info toast ("Bỏ qua 2 file không phải .mp3"). Dragging is never required: the button does the same job (WCAG 2.5.7).

### 4.7 Upload queue `.queue` / `.qitem`
Each row is a grid: stage icon (18 px) · file name (13 px medium, ellipsized, `title`=full name) · stage chip on the right. A meta line underneath, and an optional progress bar.

| Stage | Icon | Chip text | Chip color | Meta line |
|---|---|---|---|---|
| queued | clock | Đang chờ | neutral | size |
| analysing | spinner (rotating) | Đang phân tích | info | size + indeterminate bar (info) |
| uploading | upload | Đang tải lên · 64 % (or no % with stage-level progress) | accent | size + determinate bar (accent) |
| done | check-circle | Hoàn tất | success | small quality badge + link "Xem" (opens detail) |
| duplicate | copy | Trùng lặp | neutral, outlined | "Đã có trong thư viện: <title link>" (not an error) |
| error | alert-circle | Lỗi · `CODE` (mono) | danger; row gets a faint danger tint | Vietnamese message |

Above the list: the "Hàng đợi" label, the ghost xs button "Xoá mục đã xong", and a summary line `role="status"` "8 file · 2 xong · 1 trùng · 2 lỗi · 3 đang xử lý". Rows keep their order. The library refreshes on each `done`.

Error messages (code → text):

| Code | Message |
|---|---|
| `INVALID_FORMAT` | Nội dung không phải MP3 hợp lệ. |
| `TOO_LARGE` | File quá lớn (tối đa 200 MB, dài tối đa 2 giờ). |
| `TOO_SHORT` | File ngắn hơn 1 giây. |
| `SILENT` | File chỉ có im lặng. |
| `TIMEOUT` | Phân tích quá 5 phút, đã huỷ. |
| `INTERNAL` | Lỗi nội bộ khi xử lý file. |
| `STORAGE` | Không ghi được lên R2. Thư viện chưa thay đổi. |

### 4.8 Progress `.progress`
4 px track (`--color-border`), radius full. Determinate: an accent fill with `width` and a width transition; use `role="progressbar"` + `aria-valuenow/min/max`. Indeterminate: a 30 % info-colored bar sliding linearly over 1.2 s, and a static half-opacity full bar under reduced motion.

### 4.9 Dialog `.dialog` (Edit, Delete confirm)
Use native `<dialog>` + `showModal()`, which gives the focus trap, Esc and inert background for free. Style `::backdrop` with `--color-scrim`. Panel: raised surface, border, radius lg, shadow-lg, 560 px wide (`.dialog--sm` 420 px). Head: title 16 px display semibold + one-line description (muted) + ✕ ghost icon. Body scrolls. Footer: a top divider, an optional left note, and buttons on the right (secondary Huỷ, then the primary or danger action).

- **Sửa thông tin bài hát:** a cover row (72 px cover + "Ảnh bìa" + help "JPG hoặc PNG…" + [Đổi ảnh bìa…]), then a 6-column form grid: Tiêu đề * (6) · Nghệ sĩ (3) · Nghệ sĩ album (3) · Album (4) · Ngày phát hành (2) · Thể loại (4) · Track (1) · Disc (1). Track and Disc use `inputmode="numeric"`. When the dialog opens, focus goes to Tiêu đề. Enter submits and Esc cancels. Save shows the loading button, then closes and shows a success toast. On error the dialog stays open and the message appears near the field or in a danger toast.
- **Xoá N bài hát?** (`role="alertdialog"`): the description says what is removed and that it cannot be undone. A sunken list of the "**Title** · Artist" being deleted (show at most 5 + "và N bài khác"). Buttons: [Huỷ] (**gets initial focus**) and [🗑 Xoá N bài] (danger). The same dialog serves row delete and bulk delete.

### 4.10 Form field `.field`
Label (13 px medium) above the input. Required fields get a red `*` with `aria-hidden` plus the `required` attribute. Input: 32 px, radius md, 1 px `--color-border-input`, surface fill. Hover: border goes to text-subtle. **Focus**: border `--color-focus` + a 3 px ring at 28 % focus color. **Error**: `aria-invalid="true"`, danger border, and below it `.field-error` (icon + text) linked with `aria-describedby`. Disabled: sunken fill, disabled text. Help text: 12 px subtle. IDs, keys, bucket and URL inputs use `.mono` and `spellcheck="false"`.

### 4.11 Settings form
Fields: Account ID, Access Key ID, Secret Access Key, Bucket, Public base URL (`type="url"`).
- **Secret:** shows the masked value from the backend (`••••a1b2`, mono) with help text "Đang lưu secret kết thúc bằng a1b2. Để nguyên để giữ, hoặc nhập giá trị mới." There is an eye toggle (`aria-pressed`, "Hiện secret") that switches `type` between password and text for a newly typed secret. Sending the masked value back keeps the stored secret (Task 10 rule).
- **Action row:** an inline result on the left (`role="status"`), then [Kiểm tra kết nối] (secondary) and [Lưu] (primary). Result text: ok "✓ Kết nối thành công · đọc được catalog.json (N bài)", or a danger icon + the backend message. During the test the button is loading ("Đang kiểm tra…").
- **First run** (settings incomplete): open Settings directly with the info banner "Chưa cấu hình R2 · Nhập thông tin bucket để bắt đầu." The Thư viện tab still works but shows that banner.

### 4.12 Toast `.toast` (region bottom-right)
Raised surface, border-strong, 3 px left accent (`--toast-accent`: success / danger / info), icon, title (semibold) + message (muted), ✕. Success/info: `role="status"`, auto-dismiss after 4 s. Error: `role="alert"`, auto-dismiss after 8 s. Hover or focus pauses the timer. Show at most 3 at a time, newest at the bottom.

### 4.13 Library states
- **Loading:** 4–6 skeleton rows (cover block, two text bars, badge block) with a gentle pulse, and `aria-busy="true"` on the table.
- **Empty:** a 48 px accent-soft icon circle, "Thư viện đang trống", "Kéo thả file MP3 vào cửa sổ hoặc bấm Tải lên.", and [Tải lên].
- **Catalog error (`CatalogCorrupt`):** a danger banner above the table: "catalog.json bị lỗi · Không đọc được catalog trên R2. Ứng dụng sẽ không ghi gì lên bucket cho tới khi catalog được sửa." plus [Thử lại]. Disable upload, edit and delete while it shows.
- **Not configured:** the info banner from §4.11.

## 5. Formatting

| Value | Format | Example |
|---|---|---|
| Duration | `m:ss`, `h:mm:ss` ≥ 1 h | `6:30`, `1:02:05` |
| Size | MiB, 1 decimal, " MB" | `14.9 MB` |
| LUFS / LRA | 1 decimal | `-9.8` (table), `-8.1 LUFS`, `4.6 LU` |
| True peak | 1 decimal, explicit `+` above 0 | `+0.3 dBTP`, `-1.1 dBTP` |
| Bandwidth | kHz, 2 decimals | `17.15 kHz` |
| Sample rate | kHz | `44.1 kHz` |
| Sample counts | integer grouped with a narrow no-break space (U+202F) | `10 010 700` |
| Dates | `dd/MM/yyyy HH:mm`, local time | `21/11/2026 10:42` |

Decimals use a dot, matching the `format.ts` tests in Task 12. All numeric cells get `.num` and are right-aligned in tables.

## 6. Icons

Inline SVG, a 24 px grid drawn with a 1.8 stroke and round caps and joins, in the style of Lucide. They are defined once as a `<symbol>` sprite (see the top of `mockup.html`): upload, library, settings, music, x, check, check-circle, alert-circle, alert-triangle, info, clock, loader, copy, pencil, trash, refresh, eye, image, wave, file-audio, arrow-right, cloud. They render at 16 px (20 px for `.icon--lg`) with `aria-hidden="true"`. No emoji anywhere in the app. The ⚠/✓ characters in this doc stand for the SVG icons.

## 7. Accessibility checklist

- Text contrast ≥ 4.5:1 and UI boundaries ≥ 3:1, in both themes (checked).
- A visible 2 px focus ring on every control. Never remove it without replacing it. Inputs use the border + ring variant.
- Quality, stage, connection and error states always carry text and an icon or bars, never color alone.
- Everything works from the keyboard (§4.3 shortcuts, dialogs via `<dialog>`). Drag-and-drop has a button alternative.
- Queue summary and test results use `role="status"`. Error toasts use `role="alert"`.
- `prefers-reduced-motion` is honored (tokens + spinner, progress and skeleton rules).
- `lang="vi"` on `<html>`.

## 8. Implementation notes for Tasks 12–13

- Import order: `tokens.css`, then `app.css` (the lifted component CSS). Put no raw colors in `app.css`.
- The mockup's `.mk-*` classes and the `.is-focus` demo class exist only for the mockup. `.is-focus` mirrors `:focus-visible`, and you can keep it or drop it.
- In the app, `.scrim` and `.toast-region` are `position: fixed`. The mockup keeps them inside frames with `transform` on `.mk-frame`.
- Cover gradients in the mockup are fake art. Real covers come from `coverUrl` (the CSP allows `img-src https:`).
