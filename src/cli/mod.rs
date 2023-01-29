mod restore;

use std::process;

use crate::SubCommand;

pub fn run_subcommands(subcommands: Option<SubCommand>) {
    if subcommands.is_none() {
        return;
    }

    let subcommands = subcommands.unwrap();

    match subcommands {
        SubCommand::Restore { path, db } => {
            restore::restore_snapshot(path, db);
        }
    }

    process::exit(0);
}
