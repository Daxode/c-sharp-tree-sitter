use std::fs;
use std::path::Path;

fn main() -> Result<(), minreq::Error> {
    // Array of paths to the tree-sitter source files
    let sources = vec![
        (
            "https://raw.githubusercontent.com/daxode/tree-sitter/master/highlight/src/",
            "c_lib.rs",
        ),
        (
            "https://raw.githubusercontent.com/daxode/tree-sitter/master/highlight/src/",
            "lib.rs",
        ),
        (
            "https://raw.githubusercontent.com/daxode/tree-sitter/master/highlight/src/",
            "lib/binding_rust/bindings.rs"
        ),
        (
            "https://raw.githubusercontent.com/daxode/tree-sitter-c-sharp/master/bindings/rust/",
            "lib.rs",
        ),
        (
            "https://raw.githubusercontent.com/daxode/tree-sitter-c-sharp/master/src/",
            "parser.c",
        ),
        (
            "https://raw.githubusercontent.com/daxode/tree-sitter-c-sharp/master/src/",
            "scanner.c",
        ),
        (
            "https://raw.githubusercontent.com/daxode/tree-sitter-c-sharp/master/src/",
            "tree_sitter/parser.h",
        ),
    ];

    // language sources are group with the lib name
    let lang_sources = vec![
        (3, 4, "tree_sitter_c_sharp"),
    ];
    struct LangAndIsExtern {index: usize, is_ext: bool}
    let rust_sources = vec![
        // LangAndIsExtern { index: 0, is_ext: true }, 
        // LangAndIsExtern { index: 1, is_ext: true }, 
        LangAndIsExtern { index: 2, is_ext: true }
    ];
    
    // Download files
    let out_dir = std::env::var("OUT_DIR").unwrap();
    for source in sources.iter() {
        let dest_path = Path::new(&out_dir).join(source.1);
        let response = minreq::get(source.0.to_owned() + source.1).send()?;
        let parent = dest_path.parent().unwrap();
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest_path, response.as_str()?)?;
    }
    
    // Build the C# bindings
    let mut builder = csbindgen::Builder::default();
    for source_index in rust_sources {
        let dest_path = Path::new(&out_dir).join(&sources[source_index.index].1);
        if source_index.is_ext {
            builder = builder.input_extern_file(&dest_path);
        } else {
            builder = builder.input_bindgen_file(&dest_path);
        }
    }

    // let result = builder
    //     .csharp_class_name("CSharpTreeSitter")
    //     .csharp_dll_name("c-sharp-tree-sitter")
    //     .generate_csharp_file("CSharpTreeSitter.cs");
    // println!("cargo:warning={:?}", result);
    
    // for every c source file tuple use CC to create a static library
    // for lang_index in lang_sources {
    //     let main_path = Path::new(&out_dir);
    //     let parser = main_path.to_owned().join((&sources[lang_index.0]).1);
    //     let scanner = main_path.to_owned().join((&sources[lang_index.1]).1);
    //     let include = main_path.to_owned().join("tree_sitter/parser.h");
    //     println!("cargo:warning={}", parser.to_str().unwrap());
    //     println!("cargo:warning={}", scanner.to_str().unwrap());
    //     cc::Build::new()
    //         .include("tree_sitter")
    //         .files(&[parser, scanner])
    //         .compile(lang_index.2);
    // }
    
    Ok(())
}
