//! On Windows, the icon goes into the executable as resource 1, which gpui loads.

fn main() {
    println!("cargo:rerun-if-changed=assets/icons");
    embed_resource::compile(
        "assets/icons/ely-tea-party.rc",
        embed_resource::ParamsIncludeDirs(["assets/icons"]),
    )
    .manifest_required()
    .expect("build: the icon did not go into the Windows executable");
}
