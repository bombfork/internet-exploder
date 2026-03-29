use std::collections::BTreeMap;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize)]
struct Entity {
    codepoints: Vec<u32>,
    #[allow(dead_code)]
    characters: String,
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("entities.rs");
    let mut file = BufWriter::new(File::create(dest).unwrap());

    let json = include_str!("data/entities.json");
    let entities: BTreeMap<String, Entity> = serde_json::from_str(json).unwrap();

    let mut builder = phf_codegen::Map::new();
    for (name, entity) in &entities {
        // Strip leading '&' — we look up names without it
        let key = name.strip_prefix('&').unwrap_or(name);
        let codepoints: Vec<String> = entity
            .codepoints
            .iter()
            .map(|c| format!("{c}u32"))
            .collect();
        builder.entry(key, &format!("&[{}]", codepoints.join(", ")));
    }

    writeln!(
        file,
        "static NAMED_ENTITIES: phf::Map<&'static str, &'static [u32]> = {};",
        builder.build()
    )
    .unwrap();

    println!("cargo:rerun-if-changed=data/entities.json");
}
