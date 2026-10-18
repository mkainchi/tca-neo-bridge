fn main() {
    println!("cargo:rerun-if-changed=assets");
    embed_resource::compile_for("assets/icons.rc", ["neo-daemon"], embed_resource::NONE)
        .manifest_required()
        .expect("Windows icon resources require the Windows SDK resource compiler (rc.exe)");
}
