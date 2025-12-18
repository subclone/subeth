use std::fs;
use subxt::metadata::Metadata;

fn main() {
    let bytes = fs::read("artifacts/local_metadata.scale").expect("Failed to read metadata file");
    let metadata = Metadata::decode(&mut &bytes[..]).expect("Failed to decode metadata");

    println!("Signed Extensions:");
    for ext in metadata.signed_extensions() {
        println!("- {} (type: {:?})", ext.identifier(), ext.ty());
    }
}
