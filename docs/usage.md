---
icon: lucide/mouse-pointer-2
---

# Usage

### Format Selection

The `-f` / `--format` option accepts a format selection expression to specify which streams to download. The expression syntax is derived from and heavily inspired by [yt-dlp](https://github.com/yt-dlp/yt-dlp#format-selection), consisting of one or more stream selectors combined with operators:

* **Merge (`+`)**: Downloads multiple streams (e.g., `bv+ba` to download the best video and best audio streams).
* **Fallback (`/`)**: Defines a prioritized fallback chain from left to right (e.g., `bv[height=1080]/bv[height=720]` downloads 1080p if available, otherwise 720p).
* **Stream Indices**: Streams can also be chosen directly by their 1-based index (e.g., `1+3`) as shown in the `-F` stream listing output.

By default, when no format expression is specified, the CLI defaults to **`b+s+allund`** (best video + best audio + first subtitle track + all undefined streams).

!!! info "Note for yt-dlp Users"
    Unlike `yt-dlp`, which automatically merges streams when possible, `vsd` will only merge the downloaded streams into a single output file if the `-o` / `--output` flag is explicitly specified. Otherwise, each stream is saved as a separate file.

#### Keywords

Keywords are the building blocks of any format selector. They specify the target stream type (video, audio, or subtitles) and its relative quality ranking. Shorthands like `b` and `w` automatically select a combination of stream types (video + audio).

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

#### Filters

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
| `format_id` | Numeric ID of the format |

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

#### Examples

Here are some practical examples of how to construct format selection queries for the `vsd save` command.

| Expression | Description |
|------------|-------------|
| `bv+ba` | Download the best video and the best audio stream. |
| `bv[height<=720]+ba` | Download the best video stream that is 720p or lower, and the best audio stream. |
| `bv[vcodec^=vp09]+ba` | Download the best video encoded in VP9 (codec starting with `vp09`) and the best audio. |
| `bv+allaud[language=en,es]+s` | Download the best video, *all* English and Spanish audio tracks, and the first subtitle track. |
| `ba[language=en]` | Download only the best English audio stream (no video or subtitles). |
| `bv+ba[language!=ja]` | Download the best video and the best audio stream whose language is *not* Japanese. |
| `bv[height=1080]+ba / bv[height=720]+ba / b` | Try to get 1080p video with best audio; fallback to 720p video; fallback to the best available resolution. |
| `1+3` | Download streams with index `1` and `3` as shown in the `-F` stream list. |
| `allvid+allaud` | Download all available video and audio streams. |
