fn main() {
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=../daemon/assets/bridge.ico");
    embed_resource::compile_for("assets/neo-config.rc", ["neo-config"], embed_resource::NONE)
        .manifest_required()
        .expect("Windows icon resources require the Windows SDK resource compiler (rc.exe)");
}