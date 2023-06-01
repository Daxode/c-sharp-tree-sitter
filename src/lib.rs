use tree_sitter_c_sharp::*;
use tree_sitter_highlight::*;

#[no_mangle]
pub extern "C" fn w() {
    tree_sitter_c_sharp::language();
    println!("Hello, world!");
}
