//! Reads PDFs given on stdin (one path per line): pages, text per page, renders, time, failures.
//!   find ~/Documents -name '*.pdf' | cargo run --release --example pdfcheck
use std::io::BufRead;
use std::time::Instant;

fn main() {
    let (mut ok, mut failed, mut pages, mut chars, mut rendered) = (0, 0, 0, 0usize, 0);
    let t = Instant::now();
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let path = std::path::Path::new(&line);
        let one = Instant::now();
        match truffle_extract::pdf::pages(path, |n| 0..n.min(30), 200, 1400, None) {
            Ok(p) => {
                ok += 1;
                pages += p.pages.len();
                let c: usize = p.pages.iter().map(|pg| pg.text.chars().count()).sum();
                chars += c;
                rendered += p.pages.iter().filter(|pg| pg.image.is_some()).count();
                println!("ok\t{}\t{}\t{}\t{:.0}ms\t{}", p.total, p.pages.len(), c, one.elapsed().as_secs_f64() * 1000.0, line);
            }
            Err(e) => {
                failed += 1;
                println!("FAIL\t{e}\t{line}");
            }
        }
    }
    eprintln!("{ok} ok, {failed} failed, {pages} pages ({rendered} rendered), {chars} chars, {:.1}s", t.elapsed().as_secs_f64());
}
