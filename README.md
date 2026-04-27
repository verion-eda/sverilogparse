# sverilogparse

A structural Verilog parser written in Rust, targeting the subset of [IEEE
1364-2001] (Verilog-2001) produced by synthesis tools such as
[Yosys](https://yosyshq.net/yosys/).

Similar project (C++):
[OpenTimer/Parser-Verilog](https://github.com/OpenTimer/Parser-Verilog)

## What it parses

Structural Verilog (also called a gate-level netlist) describes a circuit as a
set of module instantiations connected by wires, without behavioural constructs
like `always` or `initial` blocks. A typical file looks like:

```verilog
module top(clk, q);
  input clk;
  output q;
  wire n1;

  DFF ff0(.CK(clk), .Q(n1));
  BUF b0(.A(n1), .Z(q));
endmodule
```

The parser handles:

| Construct                 | Example                                          |
| ------------------------- | ------------------------------------------------ |
| Module declarations       | `module foo(a, b, c);`                           |
| Port declarations         | `input [3:0] addr;` / `output q;` / `inout sda;` |
| Wire declarations         | `wire [7:0] bus;`                                |
| Continuous assignments    | `assign out = a & b;`                            |
| Cell instantiations       | `AND2 u1(.A(a), .B(b), .Z(out));`                |
| Concatenation expressions | `{a[1:0], 2'b01}`                                |
| Constant literals         | `8'hFF`, `4'b1x0z`                               |
| Escaped identifiers       | `\some/unusual[name] `                           |

## Verilog attributes

[IEEE 1364-2001] introduced `(* name = value *)` attribute blocks that can
precede module declarations, port/net declarations, continuous assignments, and
cell instantiations. The [formal BNF grammar] defines `attribute_instance` as a
prefix production on all of those constructs. Tools such as Yosys use attributes
extensively — see the [Yosys attribute documentation] for the full list of
recognised keys.

This parser handles attributes in all positions that structural Verilog
requires. Currently only the `(* keep *)` attribute is tracked, setting the
`keep: bool` field on the corresponding data structure (`SVerilogModule`,
`SVerilogWireDef`, `SVerilogAssign`, `SVerilogCell`). All other attributes are
consumed and discarded with a `tracing::warn` log message.

```verilog
(* keep *)
module top(a, b);
  (* keep *) input  a;
  (* keep *) output b;
  (* keep *) assign b = a;
endmodule
```

## Usage

```rust
use sverilogparse::SVerilog;

let parsed = SVerilog::parse_str(src)?;

for (name, module) in &parsed.modules {
    for cell in &module.cells {
        println!("{}: {}", cell.cell_name, cell.macro_name);
    }
}
```

Files can also be loaded directly with `SVerilog::parse_file(path)`.

[IEEE 1364-2001]: https://ieeexplore.ieee.org/document/954909
[formal BNF grammar]: https://www.syncad.com/VeriLogger_bnf_Syntax_Verilog_2001.htm
[Yosys attribute documentation]: https://yosyshq.readthedocs.io/projects/yosys/en/latest/using_yosys/verilog.html#verilog-attributes-and-non-standard-features
