fn main() -> Result<(), minreq::Error> {
    let response = 
        minreq::get("https://raw.githubusercontent.com/daxode/tree-sitter/master/highlight/src/c_lib.rs")
            .send()?;

    // create a path to save
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = std::path::Path::new(&out_dir).join("c_lib.rs");
    
    // write the contents to the file
    std::fs::write(
        &dest_path,
        response.as_str()?,
    )?;
    
    csbindgen::Builder::default()
        .input_extern_file(&dest_path)
        .csharp_class_name("CSharpTreeSitter")
        .csharp_dll_name("c-sharp-tree-sitter")
        .csharp_use_function_pointer(true)
        .generate_csharp_file("CSharpTreeSitter.cs").unwrap();
    
    Ok(())
}