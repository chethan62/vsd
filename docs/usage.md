---
icon: lucide/mouse-pointer-2
---

# Usage

Below are some example commands. For additional usage details, see [cli reference](https://clitic.github.io/vsd/cli).

### Network Capturing

Capture network requests, playlist file links (`.m3u8`, `.mpd`), and subtitle paths directly from an automated browser instance.

* **Basic request capturing:**
  ```bash
  vsd capture <url> --save-cookies
  ```
  !!! info
      The saved cookies will be written to `cookies.txt` and can be used as `--cookies cookies.txt` in subsequent download sub-commands.

* **Headless request capturing (without GUI window):**
  ```bash
  vsd capture <url> --headless --save-cookies
  ```

---

### Downloading & Format Selection (`save`)

Download DASH and HLS video streams with powerful format selection inspired by yt-dlp.

* **Basic playlist download (best video + best audio + subtitle):**
  ```bash
  vsd save <url> -o video.mp4
  ```
  !!! info
      Add `-i, --interactive` to open a styled stream selection menu.

* **List available streams:**
  ```bash
  vsd save <url> -F
  ```

* **Select streams by index (from `-F` output):**
  ```bash
  vsd save <url> -f "1+3" -o video.mp4
  ```

* **Select 720p or lower video with best audio:**
  ```bash
  vsd save <url> -f "bv[height<=720]+ba" -o video.mp4
  ```

* **Select English audio, skip subtitles:**
  ```bash
  vsd save <url> -f "bv+ba[lang=en]" -o video.mp4
  ```

* **Select all English and French audio tracks:**
  ```bash
  vsd save <url> -f "bv+allaud[lang=en,fr]+s" -o video.mp4
  ```

* **Fallback: try 1080p, otherwise 720p:**
  ```bash
  vsd save <url> -f "bv[height=1080]+ba / bv[height=720]+ba" -o video.mp4
  ```

* **Download only audio:**
  ```bash
  vsd save <url> -f "ba" -o audio.mp4
  ```

* **Clip specific timeline sections (accurate to segments):**
  ```bash
  vsd save <url> --clip 01:00-01:30 -o clip.mp4
  ```

* **Tune concurrent download threads (speed up downloads):**
  ```bash
  vsd save <url> --threads 8 -o video.mp4
  ```

---

### Format Selection Reference

The `-f` / `--format` flag accepts an expression with the following syntax:

**Keywords:**

| Keyword | Aliases | Meaning |
|---------|---------|---------|
| `b` | `best` | Best video + audio + sub + all undefined |
| `w` | `worst` | Worst video + audio + sub + all undefined |
| `bv` | `bestvideo` | Best video stream |
| `ba` | `bestaudio` | Best audio stream |
| `s` | `sub` | A subtitle stream |
| `bv*` | — | Best video (may contain muxed audio) |
| `ba*` | — | Best audio (may contain muxed video) |
| `wv` | `worstvideo` | Worst video stream |
| `wa` | `worstaudio` | Worst audio stream |
| `all` | — | All streams |
| `allvid` | — | All video streams |
| `allaud` | — | All audio streams |
| `allsub` | — | All subtitle streams |
| `allund` | — | All undefined streams |

**Filters:** `[field op value]` — appended to keywords.

| Field | Aliases | Type |
|-------|---------|------|
| `height` | `h`, `res` | number |
| `width` | `w` | number |
| `fps` | `framerate` | f32 |
| `bandwidth` | `bw`, `tbr` | kbps |
| `codec` | `codecs` | string |
| `lang` | `language` | string |
| `channels` | `ch` | f32 |

**Operators:** `=`, `!=`, `<=`, `>=`, `<`, `>`, `*=` (contains), `^=` (starts with), `$=` (ends with)

!!! tip
    Use comma-separated values with `=` for OR matching: `[lang=en,fr]` matches English or French.

**Combining:**

- `+` merges streams: `bv+ba+s`
- `/` provides fallback: `bv[height=1080]+ba / bv[height=720]+ba`
- Omit a type to skip it: `bv+ba` skips subtitles

---

### Decryption & DRM (`save`, `decrypt`, `license`)

Decrypt protected content by supplying keys directly or querying CDM servers.

* **Download DRM stream with known keys:**
  ```bash
  vsd save "https://media.axprod.net/TestVectors/Dash/protected_dash_1080p_h264_singlekey/manifest.mpd" \
      --keys "4060a865887842679cbf91ae5bae1e72:fc35340837310cc0fb53de97e22a69e0" \
      -o video.mp4
  ```

* **Decrypt a local fragmented CENC MP4 file:**
  ```bash
  vsd decrypt --key "fc35340837310cc0fb53de97e22a69e0" encrypted-video.mp4 -o decrypted.mp4
  ```

* **Decrypt with a separate initialisation segment:**
  ```bash
  vsd decrypt --key "fc35340837310cc0fb53de97e22a69e0" --init init.mp4 encrypted-video.mp4 -o decrypted.mp4
  ```

* **Request keys from Widevine license servers using a CDM device file:**
  ```bash
  vsd license --widevine-url "https://cwip-shaka-proxy.appspot.com/no_auth" --widevine-device device.wvd video-init.mp4
  ```

---

### Post-Processing (`extract`, `merge`)

Manipulate media streams and extract metadata easily.

* **Extract subtitles (`wvtt` or `stpp`) from fragmented MP4:**
  ```bash
  vsd extract media-subs.mp4 -o subtitles.srt
  ```

* **Merge multiple fragmented media segments:**
  ```bash
  vsd merge segment_*.m4s --type binary -o merged.mp4
  ```

* **Parse playlist to structured JSON metadata:**
  ```bash
  vsd save <url> --parse > parsed-playlist.json
  ```
  !!! info
      View the [JSON schema](https://github.com/clitic/vsd/blob/main/vsd/src/playlist/types.rs) of the output metadata.
