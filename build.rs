use std::io::Write;

const LOGO: u32 = 256;

fn main() {
    println!("cargo:rerun-if-changed=assets/linusware.svg");
    let svg = std::fs::read("assets/linusware.svg").expect("assets/linusware.svg");
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default()).expect("svg");
    let size = tree.size();
    let s = LOGO as f32 / size.width().max(size.height());
    let mut pixmap = resvg::tiny_skia::Pixmap::new(LOGO, LOGO).unwrap();
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s, s), &mut pixmap.as_mut());

    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    enc.write_all(pixmap.data()).unwrap();
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("logo.z"), enc.finish().unwrap()).unwrap();
}
