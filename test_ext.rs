use std::path::Path;
fn main() {
    let p = Path::new("tests/alive2/foo.ori").with_extension("preopt.ll");
    println!("{:?}", p);
}
