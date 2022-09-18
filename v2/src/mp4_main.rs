use std::fs::File;

use anyhow::Result;
use mp4::MediaType;

fn main() -> Result<()> {
    let f = File::open("../img/bad_apple.mp4")?;
    let video = mp4::read_mp4(f)?;
    for (trackid, track) in video.tracks() {
        println!("track {trackid}: media type: {}, box type: {}, duration {:?}", track.media_type()?, track.box_type()?, track.duration());
        println!("{track:#?}");
        let avc1 = track.trak.mdia.minf.stbl.stsd.avc1.as_ref().expect("not a h264 video");
        let size = (avc1.width, avc1.height);

        match track.media_type()? {
            MediaType::AAC => {} // audio (https://docs.rs/symphonia/latest/symphonia/)
            _typ @ (MediaType::H264 | MediaType::H265 /* HVEC */ | MediaType::VP9) => {} // video
            MediaType::TTXT => {} // subtitles
        }
    }

    Ok(())
}