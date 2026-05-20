use std::{
    error::Error,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    sync::LazyLock,
};
use vsd_mp4::decrypt::CencDecrypter;

const VIDEO_KEY: &str = "100b6c20940f779a4589152b57d2dacb";
const AUDIO_KEY: &str = "3bda3329158a4789880816a70e7e436d";

static SAMPLES_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/samples"));

static OUTPUT_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/vsd-mp4-samples");
    fs::create_dir_all(&dir).ok();
    dir
});

macro_rules! sample {
    ($test_name: ident, $scheme: literal, $track: literal) => {
        #[test]
        fn $test_name() -> Result<(), Box<dyn Error>> {
            let init_data = fs::read(SAMPLES_DIR.join(concat!($scheme, "/", $track, "_init.mp4")))?;
            let segment_data = fs::read(SAMPLES_DIR.join(concat!($scheme, "/", $track, "_1.m4s")))?;

            let decrypted = CencDecrypter::new(if $track == "video" {
                VIDEO_KEY
            } else {
                AUDIO_KEY
            })?
            .decrypt(segment_data, Some(init_data.as_slice()))?;

            let mut f = File::create(OUTPUT_DIR.join(concat!($scheme, "-", $track, ".mp4")))?;
            f.write_all(&init_data)?;
            f.write_all(&decrypted)?;
            Ok(())
        }
    };
}

sample!(test_cenc_video, "cenc", "video");
sample!(test_cenc_audio, "cenc", "audio");
sample!(test_cens_video, "cens", "video");
sample!(test_cens_audio, "cens", "audio");
sample!(test_cbc1_video, "cbc1", "video");
sample!(test_cbc1_audio, "cbc1", "audio");
sample!(test_cbcs_video, "cbcs", "video");
sample!(test_cbcs_audio, "cbcs", "audio");
