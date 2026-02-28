use anyhow::Result;
use clap_complete::{Shell, generate};
use std::io;

fn main() -> Result<()> {
    let mut cmd = tesuji::cli::binary_command();
    let matches = cmd.clone().get_matches();

    if let Some(&shell) = matches.get_one::<Shell>("completions") {
        generate(shell, &mut cmd, "tesuji", &mut io::stdout());
        return Ok(());
    }

    let file = matches.get_one::<String>("file").map(String::as_str);
    tesuji::cli::run(file)
}
