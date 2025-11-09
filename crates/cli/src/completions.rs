use clap::{Command, Arg};
use clap_complete::{Shell};
use std::io;

pub fn generate(shell: Shell) {
    let mut cmd = Command::new("ai")
        .about("AI Agent CLI")
        .arg(Arg::new("query").help("Query to process").index(1));

    clap_complete::generate(shell, &mut cmd, "ai", &mut io::stdout());
}
