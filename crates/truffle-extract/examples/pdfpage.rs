fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a[2].parse().unwrap();
    let p = truffle_extract::pdf::pages(std::path::Path::new(&a[1]), |t| (n - 1).min(t)..n.min(t), 200, 1400, None).unwrap();
    for pg in p.pages {
        println!("--- page {} of {} ({} chars, image: {})\n{}", pg.number, p.total, pg.text.chars().count(), pg.image.is_some(), pg.text);
    }
}
