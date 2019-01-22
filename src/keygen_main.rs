use clap::{App, Arg};
use buffett::wallet::gen_keypair_file;
use std::error;

#[no_mangle]
pub extern "C" fn keygen_main_entry() -> Result<(), Box<error::Error>> {
    println!("Keymaker!");
    let matches = clap::App::new("buffett-keygen")
        .version(crate_version!())
        .arg(
            Arg::with_name("outfile")
                .short("o")
                .long("outfile")
                .value_name("PATH")
                .takes_value(true)
                .help("Path to generated file"),
        ).get_matches();

    let mut path = dirs::home_dir().expect("home directory");
    let outfile = if matches.is_present("outfile") {
        matches.value_of("outfile").unwrap()
    } else {
        path.extend(&[".config", "solana", "id.json"]);
        path.to_str().unwrap()
    };

    let serialized_keypair = gen_keypair_file(outfile.to_string())?;
    if outfile == "-" {
        println!("{}", serialized_keypair);
    }
    Ok(())
}
