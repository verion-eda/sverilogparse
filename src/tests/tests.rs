//! unit tests for sverilogparse

use sverilogparse::*;

const VERILOG_SIMPLE: &str = include_str!("simple.v");
const VERILOG_NOT_CONNECTED: &str = include_str!("not_connected.v");
const VERILOG_KEEP_ATTRS: &str = include_str!("keep_attrs.v");
const VERILOG_VECTOR: &str = include_str!("vector.v");

#[test]
fn test_simple() {
    let parsed = SVerilog::parse_str(VERILOG_SIMPLE).expect("parse error");
    println!("Parsed is: {parsed:?}");
    println!("Restructure: \n{parsed}");
    assert_eq!(
        format!("{parsed}"),
        "\
module simple(inp1, inp2, ispd_clk, out);
  input inp1;
  input inp2;
  input ispd_clk;
  output out;
  wire n1;
  wire n2;
  wire inp1;
  wire inp2;
  wire ispd_clk;
  (* keep *) wire out;

  na02s01 u1(.a(inp1), .b(inp2), .o(n1));
  ms00f80 f1(.d(n1), .ck(ispd_clk), .o(n2));
  in01s01 u2(.a(n2), .o(out));
endmodule
"
    );

    let module = &parsed.modules[0].1;

    // `wire out` has `(* keep *)` in simple.v.
    let kept_wires: Vec<_> = module.defs.iter().filter(|d| d.keep).collect();
    assert_eq!(kept_wires.len(), 1);
    assert_eq!(kept_wires[0].name, "out");
    assert_eq!(kept_wires[0].typ, WireDefType::Wire);

    // Unknown attribute `(* src = "..." *)` on `input inp1` is warned about
    // and does not set keep.
    let inp1 = module.defs.iter().find(|d| d.name == "inp1").unwrap();
    assert!(!inp1.keep);

    // No cells or assigns have keep in this file.
    assert!(module.cells.iter().all(|c| !c.keep));
    assert!(module.assigns.iter().all(|a| !a.keep));
}

#[test]
fn test_keep_attrs() {
    let parsed = SVerilog::parse_str(VERILOG_KEEP_ATTRS).expect("parse error");
    let (module_name, module) = &parsed.modules[0];

    assert_eq!(module_name.as_str(), "keep_attrs");
    assert!(module.keep, "module should have keep");

    let def = |name: &str| {
        module
            .defs
            .iter()
            .find(|d| d.name == name)
            .unwrap_or_else(|| panic!("wire/port '{}' not found", name))
    };

    assert!(def("a").keep, "input a should have keep");
    assert!(def("b").keep, "output b should have keep");
    assert!(def("c").keep, "inout c should have keep");
    assert!(def("d").keep, "wire d should have keep");

    assert!(def("list1").keep, "wire list1 should have keep");
    assert!(def("list2").keep, "wire list2 should have keep");

    assert!(
        !def("no_keep_port").keep,
        "no_keep_port should not have keep"
    );

    assert!(
        def("no_keep_out").keep,
        "no_keep_out should have keep (chained attrs)"
    );

    assert!(module.assigns[0].keep, "first assign should have keep");
    assert!(
        !module.assigns[1].keep,
        "second assign should not have keep"
    );

    let cell = |name: &str| {
        module
            .cells
            .iter()
            .find(|c| c.cell_name == name)
            .unwrap_or_else(|| panic!("cell '{}' not found", name))
    };
    assert!(cell("u1").keep, "cell u1 should have keep");
    assert!(!cell("u2").keep, "cell u2 should not have keep");
}

#[test]
fn test_vector() {
    let parsed = SVerilog::parse_str(VERILOG_VECTOR).expect("parse error");
    println!("Restructure: \n{parsed}");
    assert_eq!(format!("{parsed}"), "\
module aes256(VGND, VPWR, clk, key, out, state);
  input VGND;
  input VPWR;
  input clk;
  input [255:0] key;
  output [127:0] out;
  inout [127:0] state;
  wire [255:0] long_wire;

  assign long_wire = {128'b11011110101011011011xxxxxxxx1111000100111000001111000000100110100101100000011000001000001110110000101101001111100100100001001101, 128'b11111100110000100100011010101000001100100001001011100101101101100101101110110000111011101101100100010010111111010010101110011010};
  anothercell x(.in({key[32:1], 4'bxx01, 3'b01z, state}), .out(out));
endmodule
");
}

const VERILOG_ASSIGN: &str = include_str!("assign.v");

#[test]
fn test_assign() {
    let parsed = SVerilog::parse_str(VERILOG_ASSIGN).expect("parse error");
    println!("Restructure: \n{parsed}");
    assert_eq!(
        format!("{parsed}"),
        "\
module xyz(x, y, z);
  input [3:0] x;
  input y;
  output [4:0] z;
  wire [1:0] a;
  wire b;

  assign z = {x, y};
  assign z[3:1] = {x, y};
  assign z[3:1] = {x, y};
  assign z[4] = 1'b1;
  assign z[3:1] = {x, y};
  assign z[3:1] = {x, y};
  assign z = {x[1:0], y};
  assign z[2:0] = {x[1:0], y};
  assign {a, b} = {x, y};
  assign {a, b} = {x, y};
endmodule
"
    );
}

const VERILOG_PORTDEF: &str = include_str!("portdef.v");

#[test]
fn test_portdef() {
    let parsed = SVerilog::parse_str(VERILOG_PORTDEF).expect("parse error");
    println!("Restructure: \n{parsed}");
    assert_eq!(format!("{parsed}"), "\
module portdef_test_pulpino_top(clk, rst_n, clk_sel_i, clk_standalone_i, testmode_i, fetch_enable_i, scan_enable_i, spi_clk_i, spi_cs_i, spi_mode_o, uart_rx, uart_rts, uart_dtr, uart_cts, uart_dsr, gpio_in, gpio_out, gpio_dir, .gpio_padcfg({\\gpio_padcfg[31][5] , \\gpio_padcfg[30][3] , \\gpio_padcfg[30][2] , \\gpio_padcfg[30][1] }), \\masters[1].w_last );

  TIEHBWP7T35P140 U5(.Z(\\masters[1].w_last ));
  TIELBWP7T35P140 U6(.ZN(net44357));
endmodule
");
}

/// Verifies that all benchmark verilog files from <https://github.com/OpenTimer/Parser-Verilog/tree/master/benchmark> parse without error.
#[test]
fn test_open_timer_benchmarks() {
    use std::path::Path;
    let benchmarks_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/benchmarks");
    let mut entries: Vec<_> = std::fs::read_dir(&benchmarks_dir)
        .expect("benchmarks dir not found")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("v"))
        .collect();
    entries.sort();
    for path in &entries {
        let content = std::fs::read_to_string(path).unwrap_or_else(|e| {
            panic!("failed to read {}: {}", path.display(), e)
        });
        SVerilog::parse_str(&content).unwrap_or_else(|e| {
            panic!("failed to parse {}: {}", path.display(), e)
        });
    }
    assert!(!entries.is_empty(), "no benchmark files found");
}

#[test]
fn test_allow_not_connected() {
    let parsed =
        SVerilog::parse_str(VERILOG_NOT_CONNECTED).expect("parse error");
    println!("Parsed is: {parsed:?}");
    println!("Restructure: \n{parsed}");
    assert_eq!(
        format!("{parsed}"),
        "\
module simple(inp1, inp2, ispd_clk, out);
  input inp1;
  input inp2;
  input ispd_clk;
  output out;
  wire n1;
  wire n2;
  wire inp1;
  wire inp2;
  wire ispd_clk;
  wire out;

  na02s01 u1(.a(inp1), .b(inp2), .o(n1));
  ms00f80 f1(.d(n1), .ck(ispd_clk), .o(n2));
  in01s01 u2(.a(n2));
endmodule
"
    );
}
