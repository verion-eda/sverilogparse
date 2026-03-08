// Tests that (* keep *) is recognised in every position our parser supports.

(* keep *)
module keep_attrs(a, b, c, d, no_keep_port, no_keep_out);

// keep on each port/wire declaration type
(* keep *) input  a;
(* keep *) output b;
(* keep *) inout  c;

// keep on a plain wire
(* keep *) wire d;

// keep propagates to all names in a comma-separated list
(* keep *) wire list1, list2;

// unknown attribute does NOT set keep
(* src = "test.v:1" *) input no_keep_port;

// unknown attr followed by keep: keep still wins
(* src = "test.v:2" *) (* keep *) output no_keep_out;

// keep on continuous assignment
(* keep *) assign b = a;

// no keep on this assignment
assign c = d;

// keep on cell instantiation
(* keep *) some_cell u1(.in(a), .out(b));

// no keep on this cell
other_cell u2(.in(c), .out(d));

endmodule
