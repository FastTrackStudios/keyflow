//! Dump a `.musx` file's raw EnigmaXML (the decrypted Finale document) —
//! for finding what the MusicXML converter does not carry over yet.
//!
//! Usage: `cargo run -p keyflow-musx --example enigma -- <in.musx> [out.xml]`

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("usage: enigma <in.musx> [out.xml]");
    let musx = std::fs::read(&input).expect("read input");
    let enigma = keyflow_musx::musx_to_enigmaxml(&musx).expect("decode musx");
    match args.next() {
        Some(out) => std::fs::write(out, enigma).expect("write output"),
        None => print!("{}", String::from_utf8_lossy(&enigma)),
    }
}
