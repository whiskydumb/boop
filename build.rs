#[cfg(windows)]
fn main() {
    use std::fs::File;
    use std::io::BufWriter;
    use std::path::PathBuf;

    let png = "assets/icon.png";
    println!("cargo:rerun-if-changed={png}");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let ico_path = out_dir.join("icon.ico");
    let rc_path = out_dir.join("icon.rc");

    let source = image::open(png)
        .expect("failed to open assets/icon.png")
        .to_rgba8();
    let (width, height) = source.dimensions();
    let side = width.min(height);
    let square =
        image::imageops::crop_imm(&source, (width - side) / 2, (height - side) / 2, side, side)
            .to_image();

    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16u32, 32, 48, 64, 128, 256] {
        let resized =
            image::imageops::resize(&square, size, size, image::imageops::FilterType::Lanczos3);
        let entry = ico::IconImage::from_rgba_data(size, size, resized.into_raw());
        dir.add_entry(ico::IconDirEntry::encode(&entry).expect("failed to encode ico entry"));
    }
    dir.write(BufWriter::new(
        File::create(&ico_path).expect("failed to create icon.ico"),
    ))
    .expect("failed to write icon.ico");

    let rc = format!(
        "1 ICON \"{}\"\n",
        ico_path.display().to_string().replace('\\', "\\\\")
    );
    std::fs::write(&rc_path, rc).expect("failed to write icon.rc");

    embed_resource::compile(&rc_path, embed_resource::NONE);
}

#[cfg(not(windows))]
fn main() {}
