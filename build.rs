fn main() {
    println!("cargo:rerun-if-changed=logo-azy-core.png");
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app.manifest");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let icon = out.join("azy-core.ico");
    let logo = image::open("logo-azy-core.png")
        .expect("Logo missing")
        .resize(256, 256, image::imageops::FilterType::Lanczos3);
    logo.save_with_format(out.join("logo-ui.png"), image::ImageFormat::Png)
        .expect("Cannot create UI logo");
    logo.save_with_format(&icon, image::ImageFormat::Ico)
        .expect("Cannot create icon");
    let rc = format!(
        "1 ICON \"{}\"\n1 24 \"{}\"\n{}",
        icon.display().to_string().replace('\\', "/"),
        std::env::current_dir()
            .unwrap()
            .join("assets/app.manifest")
            .display()
            .to_string()
            .replace('\\', "/"),
        std::fs::read_to_string("assets/app.rc").unwrap()
    );
    let resource = out.join("app.rc");
    std::fs::write(&resource, rc).unwrap();
    embed_resource::compile(resource, embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
