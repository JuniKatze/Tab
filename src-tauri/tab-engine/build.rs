fn main() {
    println!("cargo:rerun-if-changed=src/style.css");
    println!("cargo:rerun-if-changed=fonts/NotoSerifCJKsc-Regular.otf");
}
