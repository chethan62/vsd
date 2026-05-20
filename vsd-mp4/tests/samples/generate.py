import os
import subprocess

VIDEO_KID = "eb676abbcb345e96bbcf616630f1a3da"
VIDEO_KEY = "100b6c20940f779a4589152b57d2dacb"
AUDIO_KID = "63cb5f7184dd4b689a5c5ff11ee6a328"
AUDIO_KEY = "3bda3329158a4789880816a70e7e436d"
VIDEO_TAG = "VIDEO_TAG"
AUDIO_TAG = "AUDIO_TAG"

def run_command(cmd):
    try:
        subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except subprocess.CalledProcessError as e:
        print(f"Error running command: {e}")
        exit(1)

def main():
    print("Generating test.mp4")
    cmd = [
        "ffmpeg", "-hide_banner", "-y",
        "-f", "lavfi", "-i", "testsrc=duration=5:size=1920x1080:rate=24",
        "-f", "lavfi", "-i", "sine=frequency=440:duration=5",
        "-c:v", "libx264", "-c:a", "aac", "-b:a", "128k", "test.mp4"
    ]
    run_command(cmd)

    schemes = ["cenc", "cens", "cbc1", "cbcs"]

    for scheme in schemes:
        print(f"Generating {scheme}")
        cmd = [
            "packager",
            f"input=test.mp4,stream=video,init_segment={scheme}/video_init.mp4,segment_template={scheme}/video_$Number$.m4s,drm_label={VIDEO_TAG}",
            f"input=test.mp4,stream=audio,init_segment={scheme}/audio_init.mp4,segment_template={scheme}/audio_$Number$.m4s,drm_label={AUDIO_TAG}",
            "--clear_lead", "0",
            "--enable_raw_key_encryption",
            "--keys", f"label={VIDEO_TAG}:key_id={VIDEO_KID}:key={VIDEO_KEY},label={AUDIO_TAG}:key_id={AUDIO_KID}:key={AUDIO_KEY}",
            "--protection_scheme", scheme,
            "--segment_duration", "10"
        ]
        run_command(cmd)

    if os.path.exists("test.mp4"):
        os.remove("test.mp4")

if __name__ == "__main__":
    main()