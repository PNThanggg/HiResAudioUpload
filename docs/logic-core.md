# Logic core: xử lý MP3 ở backend trước khi phục vụ cho app

Tài liệu chỉ mô tả **logic**: các bước, quy tắc, công thức và ngưỡng. Không có code triển khai.
Các ngưỡng trong tài liệu đã được kiểm chứng trên file test (xem mục 10).

---

## 0. Nguyên tắc

1. **Không bao giờ encode lại audio.** File phục vụ phải là stream MP3 gốc: hoặc copy nguyên byte, hoặc remux (copy stream, chỉ thay phần vỏ chứa).
2. **Backend chỉ đo đạc, không sửa âm thanh.** Loudness, peak, gapless đều được lưu thành số. Việc áp dụng các con số đó là của app.
3. **Mọi file đều định danh bằng hash nội dung,** vì vậy nội dung không bao giờ đổi, có thể cache vĩnh viễn và dedupe miễn phí.
4. **Mỗi bài chỉ decode một lần** để lấy mọi chỉ số phân tích.

| Không làm | Lý do |
|---|---|
| Chuyển MP3 sang FLAC/WAV | Dữ liệu MP3 đã mất không lấy lại được, file chỉ to lên 4–5 lần |
| Upsample lên 24/96 ở backend | App đã upsample lúc phát. Làm ở backend chỉ tốn băng thông |
| Encode lại MP3/AAC | Lossy → lossy mất thêm chất lượng |
| Normalize trực tiếp vào file | Không đảo ngược được, lại làm hỏng tính toán headroom ở app |
| Tin vào con số bitrate | Bản 128 kbps có thể đã bị encode lại thành 320 |

---

## 1. Luồng tổng thể

```
Upload → staging → [job] Kiểm tra → Dedupe → Gapless → Phân tích (1 lần decode)
       → Tạo file phục vụ → Tách ảnh bìa → Upload storage → Ghi DB → Xoá staging
App: lấy metadata → tải audio theo id (cache) → áp gain / gapless → phát
```

- Client upload lên **bucket staging** bằng presigned URL, rồi báo cho API. API đẩy job vào hàng đợi.
- Worker xử lý tuần tự các bước ở mục 2–8. **Bước nào lỗi thì dừng cả job,** không ghi dữ liệu dở dang.
- Thứ tự ghi: **upload storage trước, ghi DB sau cùng.** Job chạy lại giữa chừng sẽ không để lại bản ghi trỏ tới file không tồn tại.

---

## 2. Kiểm tra đầu vào

Luôn xác định định dạng bằng cách **đọc nội dung file** (probe). Không tin đuôi tên file hay Content-Type do client gửi lên.

| Điều kiện | Quy tắc |
|---|---|
| Kích thước | ≤ 200 MB |
| Stream audio | Có ít nhất 1 stream. Nếu có nhiều thì lấy stream đầu tiên |
| Codec | Phải là MP3. Codec khác thì từ chối, hoặc chuyển sang nhánh xử lý riêng |
| Thời lượng | ≥ 1 giây và ≤ 2 giờ |
| Kênh | Mono hoặc stereo |
| Sample rate | 32 / 44.1 / 48 kHz (và 8–24 kHz với MPEG-2/2.5) |
| Bitrate | CBR: lấy bitrate của stream. VBR: lấy bitrate trung bình của container |
| Tag | Chỉ lấy title, artist, album, album_artist, date, genre, track, disc. Key không phân biệt hoa thường, cắt khoảng trắng hai đầu |
| Ảnh bìa | Ghi nhận nếu có stream ảnh được đánh dấu là "attached picture" (JPEG hoặc PNG) |

Mỗi job có **timeout cứng** (ví dụ 5 phút). Hết thời gian thì kill tiến trình decode và đánh job là lỗi.

---

## 3. Dedupe

- Tính **SHA-256 của file gốc** (`sourceSha256`).
- Nếu DB đã có `sourceSha256` này thì trả về track cũ và dừng job.
- Định danh của track (`id`) là **SHA-256 của file phục vụ**. Giá trị này khác `sourceSha256` khi file có bị remux (mục 6).

---

## 4. Gapless: đọc LAME tag

### Vì sao cần
Encoder MP3 thêm mẫu thừa ở **đầu bài** (encoder delay) và **cuối bài** (padding để đủ frame). Decoder MP3 cũng tự thêm **529 mẫu** độ trễ ở đầu. Không cắt đi thì giữa các bài liền mạch sẽ có một khoảng lặng ngắn.

### Cách tìm tag
1. Nếu file mở đầu bằng `ID3`, đọc kích thước tag ở byte 6–9. Đây là số **syncsafe**: mỗi byte chỉ dùng 7 bit, tức là kích thước = b6·2²¹ + b7·2¹⁴ + b8·2⁷ + b9. Bỏ qua 10 byte header + kích thước đó, cộng thêm 10 byte nếu cờ footer (bit 4 của byte 5) được bật.
2. Tìm frame MPEG đầu tiên: 11 bit đầu đều là 1, layer = III, và chỉ số bitrate khác 15.
3. Từ header của frame, suy ra độ dài phần side info và số mẫu mỗi frame:

| Phiên bản | Mono | Stereo | Mẫu/frame |
|---|---|---|---|
| MPEG-1 | 17 byte | 32 byte | 1152 |
| MPEG-2 / 2.5 | 9 byte | 17 byte | 576 |

4. Ngay sau 4 byte header và phần side info, kiểm tra chuỗi `Xing` (file VBR) hoặc `Info` (file CBR). Không có thì file không có thông tin gapless. File vẫn hợp lệ, chỉ là trả `gapless = null`.
5. Đọc 4 byte cờ (flags) của Xing. Lần lượt theo các bit:
   - bit 0: có số frame (4 byte) → đọc và lưu lại.
   - bit 1: có số byte (4 byte) → bỏ qua.
   - bit 2: có bảng TOC (100 byte) → bỏ qua.
   - bit 3: có chỉ số quality (4 byte) → bỏ qua.
6. Ngay sau đó là **LAME tag**:

| Offset | Độ dài | Nội dung |
|---|---|---|
| 0 | 9 | Tên encoder (`LAME3.100`, `Lavc60.31`…) |
| 10 | 1 | Lowpass ÷ 100 Hz (0 = không ghi) |
| 21–23 | 3 | 12 bit đầu = **delay**, 12 bit sau = **padding** |

### Công thức

```
trimStart    = delay + 529
trimEnd      = max(0, padding − 529)
totalSamples = frames × mẫu/frame − delay − padding     (mẫu trên mỗi kênh)
```

Ví dụ thực tế (LAME, 44.1 kHz): delay = 576, padding = 1224, nên trimStart = 1105 và trimEnd = 695. File dài 30 giây cho totalSamples = 1 323 000, đúng bằng 30 × 44 100.

---

## 5. Phân tích: một lần decode

Decode cả bài ra PCM float, và trên **cùng một luồng dữ liệu** tính 3 nhóm chỉ số. Xử lý dạng stream, không giữ cả bài trong RAM.

### 5.1 Loudness và true peak (EBU R128 / ITU-R BS.1770)

| Chỉ số | Logic |
|---|---|
| **Integrated loudness (LUFS)** | Lọc K-weighting (shelf tăng tần cao + high-pass). Tính năng lượng trên các khối 400 ms, mỗi khối chồng lên khối trước 75%. Loại các khối dưới −70 LUFS (gate tuyệt đối). Tính trung bình các khối còn lại, rồi loại tiếp các khối thấp hơn trung bình đó 10 LU (gate tương đối). Trung bình lần cuối chính là integrated loudness |
| **LRA (LU)** | Lấy các khối 3 s. Gate tuyệt đối −70 LUFS, gate tương đối −20 LU. LRA = percentile 95 − percentile 10 |
| **True peak (dBTP)** | Oversample ×4, rồi lấy giá trị tuyệt đối lớn nhất. Con số này bắt được các đỉnh nằm giữa hai mẫu, nên có thể lớn hơn 0 dBFS |

- Kết quả −inf hoặc dưới −70 LUFS (file im lặng hoặc quá ngắn) thì **từ chối file**.
- Dùng bộ đo chỉ đọc (kiểu filter `ebur128`), không dùng bộ normalize động. Bộ normalize cho cùng kết quả nhưng chậm hơn khoảng 3.6 lần.

### 5.2 Dải tần thực sự (phát hiện bản "nâng bitrate")

Ý tưởng: encoder MP3 luôn cắt bỏ tần số cao (lowpass), và mức cắt phụ thuộc bitrate. Phía trên điểm cắt gần như chỉ còn số 0. Vì vậy trên phổ trung bình sẽ có một **"vách"** rất rõ, và vị trí vách cho biết chất lượng thật của file, bất kể file đang ghi bitrate bao nhiêu.

**Thuật toán:**
1. Trộn hai kênh thành mono.
2. Lấy các khung 8192 mẫu, nhân cửa sổ Hann, tính FFT, rồi cộng dồn bình phương biên độ của từng bin. Chỉ cần phân tích 1/4 số khung (mỗi chu kỳ 32 768 mẫu lấy 1 khung) là đủ để có phổ trung bình.
3. Chia phổ thành các dải rộng 100 Hz. Tính mức trung bình (dB) của từng dải.
4. **Mức tham chiếu** = mức trung bình của dải 2–8 kHz.
5. Đi **từ Nyquist xuống**. Dải đầu tiên có mức **> tham chiếu − 75 dB** là nơi nội dung thật kết thúc. Mép trên của dải đó chính là `effectiveBandwidthHz`.
6. Nếu mức tham chiếu gần như bằng 0 thì file im lặng, bỏ qua bước này.

Ngưỡng 75 dB được chọn vì hai lẽ. Nhạc thật ở vùng 16–20 kHz hiếm khi thấp hơn mức tham chiếu quá 50–60 dB. Còn phía trên lowpass, mức tín hiệu nằm ở khoảng −120 dB trở xuống. Ngưỡng 75 dB nằm gọn ở giữa hai vùng này.

**Phân loại:**

| Dải tần thực | `qualityHint` | Thường là |
|---|---|---|
| ≥ 19.5 kHz (hoặc ≥ Nyquist − 1 kHz) | `full` | 256–320 kbps thật |
| 18 – 19.5 kHz | `high` | VBR V0–V2, khoảng 192–256 kbps |
| 15.5 – 18 kHz | `medium` | 128–160 kbps, hoặc **bản 128 bị encode lại thành 320** |
| < 15.5 kHz | `low` | Nguồn kém |

### 5.3 Waveform

1. Chia bài thành các khối 1024 mẫu. Mỗi khối lấy **peak tuyệt đối lớn nhất của cả hai kênh**.
2. Gom các giá trị đó lại thành **800 điểm**: mỗi điểm là giá trị lớn nhất của nhóm khối tương ứng. Kẹp kết quả về khoảng [0, 1] và làm tròn 3 chữ số.

---

## 6. File phục vụ

| Trường hợp | Xử lý |
|---|---|
| Ảnh bìa nhúng nhỏ hoặc không có | Phục vụ **bản gốc nguyên byte** |
| Ảnh bìa nhúng lớn (ví dụ > 200 KB) | **Remux**: chỉ giữ stream audio, copy stream (không encode), bỏ toàn bộ tag và ảnh, ghi lại header Xing/LAME |

Điều kiện bắt buộc sau khi remux, tức là phải kiểm chứng được:
- PCM decode ra **giống hệt từng mẫu** so với file gốc (so hash của PCM).
- LAME tag vẫn còn, và delay/padding **không đổi**.

**LAME tag luôn đọc từ file gốc,** trước khi remux.

---

## 7. Ảnh bìa và lưu trữ

- **Ảnh bìa:** tách stream ảnh ra một file riêng, **copy nguyên** (không nén lại). JPEG thì lưu `.jpg`, PNG thì lưu `.png`. Định dạng khác thì bỏ qua.
- **Key lưu trữ:** `audio/<id>.mp3` và `cover/<id>.<ext>`.

| Object | Content-Type | Cache-Control |
|---|---|---|
| Audio | `audio/mpeg` | `public, max-age=31536000, immutable` |
| Ảnh bìa | `image/jpeg` hoặc `image/png` | `public, max-age=31536000, immutable` |

**CDN:**
- Bật Range request để app seek được khi phát trực tiếp từ mạng.
- **Không nén gzip/brotli** cho audio và ảnh, vì các file này đã được nén sẵn.
- Cần kiểm soát quyền truy cập thì dùng signed URL có hạn, vẫn giữ cache ở edge.

---

## 8. Metadata lưu và trả về

| Nhóm | Trường |
|---|---|
| Định danh | `id`, `sourceSha256`, `audioKey`, `coverKey`, `sizeBytes` |
| Kỹ thuật | `codec`, `bitrate`, `vbr`, `sampleRate`, `channels`, `durationMs` |
| Tag | `title`, `artist`, `album`… |
| Loudness | `integratedLufs`, `truePeakDbtp`, `lra` |
| Chất lượng | `effectiveBandwidthHz`, `qualityHint` |
| Gapless | `trimStart`, `trimEnd`, `totalSamples` (null nếu không có LAME tag) |
| UI | `waveform` (800 giá trị). **Chỉ trả ở endpoint chi tiết,** không trả trong danh sách |
| Vận hành | `analysisVersion`, `createdAt` |

Khi đổi thuật toán phân tích, **tăng `analysisVersion`** rồi chạy lại phân tích trên file đã lưu. Không cần người dùng upload lại.

---

## 9. Phía app dùng metadata

### 9.1 Tải về
- Tải hết file về cache theo `id`, đối chiếu `sizeBytes` rồi mới phát từ file local. File MP3 chỉ khoảng 8–10 MB.
- Cần phát sớm thì tải dạng Range tuần tự, và bắt đầu phát khi đã có khoảng 10 giây đầu.

### 9.2 Gain cho từng bài (gộp vào tầng preamp của core phát nhạc)

```
target  = −16 LUFS                        (IEM: chừa headroom cho EQ)
ceiling = −1 dBTP

g = target − integratedLufs                                  // cân bằng âm lượng
g = min(g, ceiling − truePeakDbtp − eqPeakBoost − ispHeadroom) // không để đỉnh vượt trần
g = clamp(g, −24, +12)
gainTổng = g + eqPreamp                                      // một phép nhân 64-bit
```

| Bài | integratedLufs | truePeak | g |
|---|---|---|---|
| Master "nóng" | −9.9 | +0.4 | −6.1 dB (giảm, đủ chỗ cho EQ boost) |
| Master nhỏ | −24 | −13 | Muốn tăng +8 dB, nhưng bị trần peak chặn lại. Ví dụ với EQ boost 6 dB và ispHeadroom −1 dB: −1 − (−13) − 6 − (−1) = **+7 dB** |

Đây là tính năng bật/tắt được ("Track gain"). Khi tắt thì `g = 0`, nhưng **vẫn giữ** preamp của EQ.

### 9.3 Gapless
- Sau khi decode: bỏ `trimStart` frame đầu tiên, rồi chỉ phát đúng `totalSamples` frame.
- Một số decoder đã tự bỏ 529 mẫu độ trễ. Cần kiểm tra trên máy thật bằng một tín hiệu sine liên tục cắt làm hai bài. Nếu sau khi cắt bị hụt mất một đoạn nhỏ ở đầu bài, dùng `trimStart − 529`.

### 9.4 Nhãn chất lượng
Hiển thị theo `qualityHint` thay cho bitrate, ví dụ "MP3 320 · Full band" hoặc "MP3 320 · nguồn ~128k".

---

## 10. Kiểm chứng ngưỡng

Thử trên pink noise stereo 30 giây, 44.1 kHz:

| File | Bitrate ghi | LUFS | True peak | Dải tần | Hint |
|---|---|---|---|---|---|
| 320 CBR thật | 320k | −23.9 | −13.0 | 20 550 Hz | full |
| 128k encode lại thành 320k | 320k | −24.2 | −13.2 | 17 150 Hz | **medium** |
| Đẩy to + limiter | 320k | −9.9 | **+0.4** | 20 550 Hz | full |
| LAME VBR V2 | ~179k | −23.8 | −12.7 | 19 250 Hz | high |
| 128 CBR | 128k | −24.2 | −13.1 | 17 150 Hz | medium |

- Tất cả các file đều đọc được gapless 1105 / 695 / 1 323 000 mẫu.
- Remux bỏ ảnh bìa và tag: PCM giống hệt file gốc, LAME tag giữ nguyên.
- Bài 4 phút: phân tích xong trong khoảng 3 giây trên 1 nhân CPU, với điều kiện decode một lần. Nếu đo loudness ở một lần decode riêng thì mất khoảng 9 giây.

---

## 11. Quy tắc vận hành

- **Số worker** đồng thời bằng số nhân CPU. Mỗi job chạy một tiến trình decode và có timeout riêng.
- **Bộ decode là bộ parse dữ liệu không tin cậy:** chạy trong container với user không có quyền root, chỉ được ra mạng tới object storage, filesystem chỉ đọc được, trừ thư mục tạm.
- **Idempotent:** dedupe theo `sourceSha256`, upload storage trước, ghi DB sau cùng.
- **Lỗi được phân loại** để client hiển thị đúng: `INVALID_FORMAT`, `TOO_LARGE`, `TOO_SHORT`, `SILENT`, `TIMEOUT`, `INTERNAL`.
- **Sau này nhận file lossless:** giữ bản lossless làm bản chính, và tuỳ chọn tạo thêm bản Opus 128 kbps cho chế độ tiết kiệm dữ liệu. **Không bao giờ** tạo bản lossless từ MP3.
