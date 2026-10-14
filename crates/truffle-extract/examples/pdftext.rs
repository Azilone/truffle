fn main() {
    let path = std::env::args().nth(1).unwrap();
    let p = truffle_extract::pdf::pages(std::path::Path::new(&path), |n| 0..n.min(1), 200, 1400, None).unwrap();
    for pg in p.pages {
        println!("--- page {} ({} chars, image: {})", pg.number, pg.text.chars().count(), pg.image.is_some());
        println!("{}", pg.text.chars().take(500).collect::<String>());
    }
}
