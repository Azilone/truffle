//! kind<TAB>path lines on stdin: image | shot | audio | video. Prints what each reader made and how long it took.
use std::io::BufRead;
use std::path::Path;
use std::time::Instant;
use truffle_extract::media::{av, image, ocr};

fn main() {
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let Some((kind, path)) = line.split_once('\t') else { continue };
        let p = Path::new(path);
        let name = p.file_name().unwrap().to_string_lossy().chars().take(40).collect::<String>();
        let t = Instant::now();
        let what = match kind {
            "image" => match image::decode(p, 1024, 256) {
                Ok(d) => format!("{}x{} from {}x{}", d.image.width, d.image.height, d.width, d.height),
                Err(e) => format!("ERR {e}"),
            },
            "shot" => match image::decode(p, 2600, 1) {
                Ok(d) => {
                    let text = ocr::text(&d.image, ocr::Script::UserProfile);
                    format!("{} chars: {:?}", text.chars().count(), text.chars().take(60).collect::<String>())
                }
                Err(e) => format!("ERR {e}"),
            },
            "audio" => {
                let mut n = 0usize;
                match av::audio(p, |b| {
                    n += b.len();
                    Ok(())
                }) {
                    Ok(has) => format!("audio={has} {:.1}s at 16 kHz", n as f64 / 16000.0),
                    Err(e) => format!("ERR {e}"),
                }
            }
            "video" => match av::info(p) {
                Ok(info) => {
                    let times: Vec<f64> = (0..).map(|i| 0.5 + 3.0 * i as f64).take_while(|t| *t < info.duration).take(20).collect();
                    let frames = av::frames(p, &times, 768, 3.0).map(|f| f.len()).unwrap_or(0);
                    let mut n = 0usize;
                    let _ = av::audio(p, |b| {
                        n += b.len();
                        Ok(())
                    });
                    format!("{:.1}s {}x{} video={} audio={} frames={} sound={:.1}s", info.duration, info.width, info.height, info.video, info.audio, frames, n as f64 / 16000.0)
                }
                Err(e) => format!("ERR {e}"),
            },
            _ => continue,
        };
        println!("{kind:5} {:>6.0}ms  {name:40}  {what}", t.elapsed().as_secs_f64() * 1000.0);
    }
}
