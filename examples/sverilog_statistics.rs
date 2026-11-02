// SPDX-FileCopyrightText: Copyright (c) Zizheng Guo. Originally from eda-infra-rs (https://github.com/gzz2000/eda-infra-rs).
// SPDX-FileCopyrightText: Copyright (c) 2026 Verion GmbH. Modifications to the
// original file.
//
// SPDX-License-Identifier: Apache-2.0
use std::env;

use sverilogparse::SVerilog;

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::DEBUG)
        .init();
    let args: Vec<String> = env::args().collect();
    assert!(args.len() == 2, "Usage: {} <sverilog_path>", args[0]);
    tracing::info!("Verilog file {}", args[1]);

    let sv = match SVerilog::parse_str(
        &std::fs::read_to_string(&args[1]).unwrap(),
    ) {
        Ok(sv) => sv,
        Err(e) => panic!("{}", e),
    };

    tracing::info!("# Modules = {}", sv.modules.len());
    tracing::info!(
        "# Cell lines = {}",
        sv.modules.iter().map(|m| m.1.cells.len()).sum::<usize>()
    );

    drop(sv);
    tracing::info!("cleaned up, exiting..")
}
