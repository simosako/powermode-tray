fn main() {
    println!("cargo:rerun-if-changed=resources.rc");
    println!("cargo:rerun-if-changed=assets/balance_fluent.ico");
    println!("cargo:rerun-if-changed=assets/performance_fluent.ico");
    println!("cargo:rerun-if-changed=assets/eco_fluent.ico");

    embed_resource::compile("resources.rc", embed_resource::NONE);
}
