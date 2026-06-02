---
icon: lucide/mouse-pointer-2
---

# Usage

Practical examples and common scenarios for the `vsd` command-line utility.

## Downloading Playlists

Use the [save] sub-command to download streams from a DASH or HLS playlist. By providing an output path, you can optionally mux them into a single file using [ffmpeg](https://github.com/Tyrrrz/FFmpegBin/releases).

```bash
vsd save "https://media.axprod.net/TestVectors/Hls/not_protected_hls_1080p_h264/manifest.m3u8" -o output.mp4
```

!!! info "Note about default format selection"
    By default, when no format expression is specified, the cli defaults to **`b+s+allund`** (best video + best audio + first subtitle track + all undefined streams). See [format selection](#format-selection) section for more info.

!!! info "Note for yt-dlp users"
    Unlike `yt-dlp`, which automatically merges streams when possible, `vsd` will only merge the downloaded streams into a single output file if the `-o` / `--output` flag is explicitly specified. Otherwise, each stream is saved as a separate file.

## Capturing Playlists

Use the [capture](https://clitic.github.io/vsd/cli/#vsd-capture) sub-command to launch an automated browser instance, intercept network requests, and extract playlist urls as `curl` commands.

!!! warning "Limitations"
    This automated capture might not work on all websites, especially those using advanced bot protection or complex authentication. In such cases, you may need to manually extract the playlist url from the network tab of your browser's developer tools.

```bash
vsd capture "https://bitmovin.com/demos/stream-test"
```

```
----------------------------------------
curl -X GET -H 'Referer: https://bitmovin.com/' -H 'User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36' -H 'sec-ch-ua: "Chromium";v="148", "Google Chrome";v="148", "Not/A)Brand";v="99"' -H 'sec-ch-ua-mobile: ?0' -H 'sec-ch-ua-platform: "Windows"' 'https://cdn.bitmovin.com/content/assets/art-of-motion-dash-hls-progressive/m3u8s/f08e80da-bf1d-4e3d-8899-f0f6155f6efa.m3u8' --compressed
----------------------------------------
curl -X GET -H 'Referer: https://bitmovin.com/' -H 'User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36' -H 'sec-ch-ua: "Chromium";v="148", "Google Chrome";v="148", "Not/A)Brand";v="99"' -H 'sec-ch-ua-mobile: ?0' -H 'sec-ch-ua-platform: "Windows"' 'https://cdn.bitmovin.com/content/assets/art-of-motion-dash-hls-progressive/m3u8s/f08e80da-bf1d-4e3d-8899-f0f6155f6efa_video_1080_4800000.m3u8' --compressed
----------------------------------------
curl -X GET -H 'Referer: https://bitmovin.com/' -H 'User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36' -H 'sec-ch-ua: "Chromium";v="148", "Google Chrome";v="148", "Not/A)Brand";v="99"' -H 'sec-ch-ua-mobile: ?0' -H 'sec-ch-ua-platform: "Windows"' 'https://cdn.bitmovin.com/content/assets/art-of-motion-dash-hls-progressive/m3u8s/f08e80da-bf1d-4e3d-8899-f0f6155f6efa_audio_1_stereo_128000.m3u8' --compressed
```

!!! tip "Testing Captured Requests"
    You can import the generated `curl` commands into API clients like [Hoppscotch](https://hoppscotch.io) to verify that the captured headers successfully retrieve the playlist.

Once the playlist url and required headers are captured, pass them to the [save] sub-command to download the streams.

```bash
vsd save -d "https://cdn.bitmovin.com/content/assets/art-of-motion-dash-hls-progressive/m3u8s/f08e80da-bf1d-4e3d-8899-f0f6155f6efa.m3u8" \
  -H "Referer: https://bitmovin.com/" \
  -f worstvideo -o output.mp4
```

## Downloading DRM-Protected Playlists

To download and decrypt DRM-protected playlist (e.g. using Widevine or PlayReady), you must obtain the decryption keys. The following examples use test vectors from [Axinom/public-test-vectors](https://github.com/Axinom/public-test-vectors).

If you attempt to save protected playlist directly without keys, the download will fail and display the required KeyIDs.

```bash
vsd save "https://media.axprod.net/TestVectors/Dash/protected_dash_1080p_h264_singlekey/manifest.mpd" \
  -f worstvideo -o widevine.mp4
```

```
Stream [vid] 1920x1080 | 3838k |  avc1.640028 |  24 fps
Stream [vid]  1280x720 | 2700k |  avc1.64001f |  24 fps
Stream [vid]   852x480 | 1551k |  avc1.64001e |  24 fps
Stream [vid]   640x360 | 1032k |  avc1.64001e |  24 fps
Stream [vid]   512x288 |  513k |  avc1.640015 |  24 fps
Stream [aud]        en |  140k |    mp4a.40.2 |   2 ch
Stream [aud]    en-low |  138k |    mp4a.40.2 |   2 ch
Stream [aud]   en-high |  138k |    mp4a.40.2 |   2 ch
Stream [sub]        de |    ?k |         wvtt |
Stream [sub]        fr |    ?k |         wvtt |
Stream [sub]        en |    ?k |         wvtt |
DrmPsh [prd] AAACJnBzc2gAAAAAmgTweZhAQoarkuZb4IhflQAAAgYGAgAAAQABAPwBPABXAFIATQBIAEUAQQBEAEUAUgAgAHgAbQBsAG4AcwA9ACIAaAB0AHQAcAA6AC8ALwBzAGMAaABlAG0AYQBzAC4AbQBpAGMAcgBvAHMAbwBmAHQALgBjAG8AbQAvAEQAUgBNAC8AMgAwADAANwAvADAAMwAvAFAAbABhAHkAUgBlAGEAZAB5AEgAZQBhAGQAZQByACIAIAB2AGUAcgBzAGkAbwBuAD0AIgA0AC4AMAAuADAALgAwACIAPgA8AEQAQQBUAEEAPgA8AFAAUgBPAFQARQBDAFQASQBOAEYATwA+ADwASwBFAFkATABFAE4APgAxADYAPAAvAEsARQBZAEwARQBOAD4APABBAEwARwBJAEQAPgBBAEUAUwBDAFQAUgA8AC8AQQBMAEcASQBEAD4APAAvAFAAUgBPAFQARQBDAFQASQBOAEYATwA+ADwASwBJAEQAPgBaAGEAaABnAFEASABpAEkAWgAwAEsAYwB2ADUARwB1AFcANgA0AGUAYwBnAD0APQA8AC8ASwBJAEQAPgA8AEMASABFAEMASwBTAFUATQA+AHkAeABsAEcAbABoAGYARAArAGEAYwA9ADwALwBDAEgARQBDAEsAUwBVAE0APgA8AC8ARABBAFQAQQA+ADwALwBXAFIATQBIAEUAQQBEAEUAUgA+AA==
DrmKid [prd] 65a86040788867429cbf91ae5bae1e72
DrmPsh [wvd] AAAAXHBzc2gAAAAA7e+LqXnWSs6jyCfc1R0h7QAAADwSEEBgqGWIeEJnnL+RrluuHnISEEBgqGWIeEJnnL+RrluuHnISEEBgqGWIeEJnnL+RrluuHnJI49yVmwY=
DrmKid [wvd] 4060a865887842679cbf91ae5bae1e72 (required)
DownLd [vid] 512x288 513k avc1.640015 24fps
Saving [vid] vsd-vid-93f4001
[ERROR] Missing decryption key for 4060a865887842679cbf91ae5bae1e72.
```

To fetch the keys, you can query a license server using the [license](https://clitic.github.io/vsd/cli/#vsd-license) sub-command. This requires a CDM device file (such as a `.wvd` file for Widevine or `.prd` for PlayReady). Also see, [How to obtain .wvd and .prd drm devices?](https://www.google.com/search?udm=50&q=How+to+obtain+.wvd+and+.prd+drm+devices%3F)

```bash
vsd license "https://media.axprod.net/TestVectors/Dash/protected_dash_1080p_h264_singlekey/manifest.mpd" \
  --widevine-device device.wvd \
  --widevine-url "https://drm-widevine-licensing.axprod.net/AcquireLicense" \
  -H "X-AxDRM-Message: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.ewogICJ2ZXJzaW9uIjogMSwKICAiY29tX2tleV9pZCI6ICI2OWU1NDA4OC1lOWUwLTQ1MzAtOGMxYS0xZWI2ZGNkMGQxNGUiLAogICJtZXNzYWdlIjogewogICAgInR5cGUiOiAiZW50aXRsZW1lbnRfbWVzc2FnZSIsCiAgICAidmVyc2lvbiI6IDIsCiAgICAibGljZW5zZSI6IHsKICAgICAgImFsbG93X3BlcnNpc3RlbmNlIjogdHJ1ZQogICAgfSwKICAgICJjb250ZW50X2tleXNfc291cmNlIjogewogICAgICAiaW5saW5lIjogWwogICAgICAgIHsKICAgICAgICAgICJpZCI6ICI0MDYwYTg2NS04ODc4LTQyNjctOWNiZi05MWFlNWJhZTFlNzIiLAogICAgICAgICAgImVuY3J5cHRlZF9rZXkiOiAid3QzRW51dVI1UkFybjZBRGYxNkNCQT09IiwKICAgICAgICAgICJ1c2FnZV9wb2xpY3kiOiAiUG9saWN5IEEiCiAgICAgICAgfQogICAgICBdCiAgICB9LAogICAgImNvbnRlbnRfa2V5X3VzYWdlX3BvbGljaWVzIjogWwogICAgICB7CiAgICAgICAgIm5hbWUiOiAiUG9saWN5IEEiLAogICAgICAgICJwbGF5cmVhZHkiOiB7CiAgICAgICAgICAibWluX2RldmljZV9zZWN1cml0eV9sZXZlbCI6IDE1MCwKICAgICAgICAgICJwbGF5X2VuYWJsZXJzIjogWwogICAgICAgICAgICAiNzg2NjI3RDgtQzJBNi00NEJFLThGODgtMDhBRTI1NUIwMUE3IgogICAgICAgICAgXQogICAgICAgIH0KICAgICAgfQogICAgXQogIH0KfQ.l8PnZznspJ6lnNmfAE9UQV532Ypzt1JXQkvrk8gFSRw" \
  --skip-playready
```

If successful, the license sub-command outputs the keyID and decryption key in `KID:KEY` format.

```
DrmPsh [wvd] AAAAXHBzc2gAAAAA7e+LqXnWSs6jyCfc1R0h7QAAADwSEEBgqGWIeEJnnL+RrluuHnISEEBgqGWIeEJnnL+RrluuHnISEEBgqGWIeEJnnL+RrluuHnJI49yVmwY=
DrmKey [wvd] 4060a865887842679cbf91ae5bae1e72:fc35340837310cc0fb53de97e22a69e0
```

Finally, pass the acquired decryption key to the [save] sub-command using the `--keys` option to download and decrypt the streams.

```bash
vsd save "https://media.axprod.net/TestVectors/Dash/protected_dash_1080p_h264_singlekey/manifest.mpd" \
  --keys "4060a865887842679cbf91ae5bae1e72:fc35340837310cc0fb53de97e22a69e0" \
  -f worstvideo -o widevine.mp4
```

## Format Selection

The `-f` / `--format` option accepts a format selection expression to specify which streams to download. The expression syntax is derived from and heavily inspired by [yt-dlp](https://github.com/yt-dlp/yt-dlp#format-selection), consisting of one or more stream selectors combined with operators:

* **Merge (`+`)**: Downloads multiple streams (e.g., `bv+ba` to download the best video and best audio streams). Merge is **lenient**, if any of the merged component streams do not exist, they are silently skipped (e.g., `b+s` is still successfully accepted even if no subtitles exist).
* **Fallback (`/`)**: Defines a prioritized fallback chain from left to right (e.g., `bv[height=1080]/bv[height=720]` downloads 1080p if available, otherwise 720p). Fallback is **strict**, a branch is only selected if every one of its merged components matches at least one stream (e.g., `bv[height=1080]+ba / bv[height=720]+ba` will fall back to the next option if the 1080p video is missing, even if the audio is present).
* **Stream Indices**: Streams can also be chosen directly by their 1-based index (e.g., `1+3`) as shown in the `-F` stream listing output. You can use the `-F` / `--list-formats` flag to list all available streams and get their corresponding IDs (under the `ID` column).

| Keyword | Alias | Description |
|---------|-------|-------------|
| `best` | `b` | Best video + best audio |
| `worst` | `w` | Worst video + worst audio |
| `bestvideo` | `bv` | Best video stream |
| `bestaudio` | `ba` | Best audio stream |
| `worstvideo` | `wv` | Worst video stream |
| `worstaudio` | `wa` | Worst audio stream |
| `sub` | `s` | First subtitle stream |
| `all` |   | All streams |
| `allvid` |   | All video streams |
| `allaud` |   | All audio streams |
| `allsub` |   | All subtitle streams |
| `allund` |   | All undefined streams |

### Filters

Filters allow you to narrow down streams by specifying conditions inside square brackets `[...]` after a keyword. Multiple filters can be chained together (e.g., `bv[height<=720][fps>=60]`).

For string fields, comparisons are case-insensitive. The `=` (equals) and `!=` (not equals) operators also support a list of comma-separated values to perform an **OR** check (e.g., `ba[language=en,fr]` to select English or French audio).

| Filter | Description |
|--------|-------------|
| `width` | Width of the video |
| `height` | Height of the video |
| `resolution` | `width`x`height` of the video |
| `language` | Language code |
| `tbr` | Average bitrate of video and audio in kbps |
| `abr` | Average audio bitrate in kbps |
| `vbr` | Average video bitrate in kbps |
| `fps` | Frame rate of the video |
| `audio_channels` | Number of audio channels |
| `acodec` | Name of the audio codec |
| `vcodec` | Name of the video codec |

| Operator | Description |
|---------|-------------|
| `=` | Equals |
| `!=` | Not equals |
| `<=` | Less than or equal to |
| `>=` | Greater than or equal to |
| `<` | Less than |
| `>` | Greater than |
| `*=` | Contains |
| `^=` | Starts with |
| `$=` | Ends with |

### Examples

Here are some practical examples of how to construct format selection queries for the `vsd save` command.

| Expression | Description |
|------------|-------------|
| `bv+ba` | Download the best video and the best audio streams. |
| `bv[height<=720]+ba` | Download the best video stream that is 720p or lower, and the best audio stream. |
| `bv[vcodec^=vp09]+ba` | Download the best video encoded in VP9 (codec starting with `vp09`) and the best audio. |
| `bv+allaud[language=en,es]+s` | Download the best video, *all* English and Spanish audio tracks, and the first subtitle track. |
| `ba[language=en]` | Download only the best English audio stream (no video or subtitles). |
| `bv+ba[language!=ja]` | Download the best video and the best audio stream whose language is *not* Japanese. |
| `bv[height=1080]+ba / bv[height=720]+ba / b` | Try to get 1080p video with best audio; fallback to 720p video; fallback to the best available resolution. |
| `1+3` | Download streams with index `1` and `3` as shown in the `-F` stream list. |
| `allvid+allaud` | Download all available video and audio streams. |

[save]: https://clitic.github.io/vsd/cli/#vsd-save
