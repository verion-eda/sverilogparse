// SPDX-FileCopyrightText: Copyright (c) Zizheng Guo. Originally from eda-infra-rs (https://github.com/gzz2000/eda-infra-rs).
// SPDX-FileCopyrightText: Copyright (c) 2026 Verion GmbH. Modifications to the original file.
// SPDX-License-Identifier: Apache-2.0
use std::{collections::HashSet, fmt, num::NonZeroUsize, str::FromStr};

use nom::{
    IResult,
    branch::alt,
    bytes::complete::{is_not, tag, take_till, take_till1, take_until},
    character::{
        complete::{char, digit1, multispace0, one_of, satisfy},
        is_alphanumeric, is_hex_digit,
    },
    combinator::{cut, map, map_res, opt, recognize, value},
    multi::{fold_many0, many0_count, separated_list0},
    sequence::{delimited, pair, preceded, tuple},
};

use super::*;

/// comment: starts with // and ends with a line.
/// do not use streaming operators here.
/// performance critical.
pub fn skip_whitespace_and_comment(mut i: &[u8]) -> IResult<&[u8], ()> {
    (i, _) = multispace0(i)?;
    while !i.is_empty() {
        if i[0] == b'/' {
            i = match value(
                (),
                tuple((
                    char('/'),
                    alt((
                        value((), pair(char('/'), is_not("\r\n"))),
                        value(
                            (),
                            tuple((char('*'), take_until("*/"), tag("*/"))),
                        ),
                    )),
                )),
            )(i) as IResult<&[u8], ()>
            {
                Ok((i, ())) => i,
                Err(_) => break,
            };
            (i, _) = multispace0(i)?;
        } else {
            break;
        }
    }
    Ok((i, ()))
}

/// Parses a single `(* ... *)` attribute block, returning the trimmed content
/// string.
fn attr_block(i: &[u8]) -> IResult<&[u8], String> {
    let (i, content) = delimited(tag("(*"), take_until("*)"), tag("*)"))(i)?;
    let content_str = std::str::from_utf8(content)
        .map_err(|_| {
            nom::Err::Error(nom::error::Error::new(
                i,
                nom::error::ErrorKind::Char,
            ))
        })?
        .trim()
        .to_owned();
    Ok((i, content_str))
}

/// Parses optional leading attribute blocks with surrounding whitespace and
/// returns whether any `(* keep *)` attribute was found.
fn leading_attrs(i: &[u8]) -> IResult<&[u8], bool> {
    let (i, attrs) =
        fold_many0(ws(attr_block), HashSet::new, |mut set, attr| {
            set.insert(attr);
            set
        })(i)?;

    let is_keep_attr =
        |attr: &String| attr == "keep" || attr.starts_with("keep ");
    for attr in &attrs {
        // Yosys emits `keep = 32'd1` (valued form); bare `keep` also accepted.
        if !is_keep_attr(attr) {
            tracing::warn!("ignoring unknown attribute: (* {} *)", attr);
        }
    }
    Ok((i, attrs.iter().any(is_keep_attr)))
}

/// a higher-order parser transforming a parser to one that
/// eats up all spaces.
pub fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], O>
where
    F: FnMut(&'a [u8]) -> IResult<&'a [u8], O>,
{
    delimited(
        skip_whitespace_and_comment,
        inner,
        skip_whitespace_and_comment,
    )
}

/// Parse an identifier.
fn ident(i: &[u8]) -> IResult<&[u8], CompactString> {
    map_res(
        alt((
            preceded(
                char('\\'),
                cut(take_till1(|c| {
                    c == b'\n' || c == b'\r' || c == b' ' || c == b'\t'
                })),
            ),
            recognize(pair(
                satisfy(|c| c.is_alphabetic() || c == '_'),
                take_till(|c| !is_alphanumeric(c) && c != b'_' && c != b'$'),
            )),
        )),
        |s| std::str::from_utf8(s).map(CompactString::from),
    )(i)
}

fn int(input: &[u8]) -> IResult<&[u8], isize> {
    map_res(
        map_res(
            recognize(preceded(opt(char('-')), digit1)),
            std::str::from_utf8,
        ),
        isize::from_str,
    )(input)
}

fn uint(input: &[u8]) -> IResult<&[u8], usize> {
    map_res(
        map_res(recognize(digit1), std::str::from_utf8),
        usize::from_str,
    )(input)
}

/// parses a constant literal.
/// if the width of the constant exceeds 128, it will be split to multiple
/// 128-bit entries.
fn literal(i: &[u8]) -> IResult<&[u8], Vec<WirexprBasic>> {
    map(
        tuple((
            uint,
            char('\''),
            one_of("bBoOdDhH"),
            take_till1(|c| {
                !is_hex_digit(c)
                    && c != b'_'
                    && c != b'x'
                    && c != b'X'
                    && c != b'z'
                    && c != b'Z'
            }),
        )),
        |(width, _, radix, hexint)| {
            use awint::ExtAwi;
            let width_nonzero = NonZeroUsize::new(width).unwrap();
            let radix = match radix {
                'b' | 'B' => 2,
                'o' | 'O' => 8,
                'd' | 'D' => 10,
                'h' | 'H' => 16,
                _ => unreachable!(),
            };

            let has_xz = hexint.iter().any(|c| matches!(*c, b'x' | b'z'));
            let (mut value, mut is_xz) = if !has_xz {
                (
                    ExtAwi::from_bytes_radix(
                        None,
                        hexint,
                        radix,
                        width_nonzero,
                    )
                    .unwrap(),
                    ExtAwi::zero(width_nonzero),
                )
            } else {
                // we need to decode the x/z into is_xz bits.
                // also we need to expand the highest-digit if it is x or z.
                let (radix_full_char, radix_bits) = match radix {
                    10 => panic!("base 10 literals cannot have x/z."),
                    2 => (b'1', 1),
                    8 => (b'7', 3),
                    16 => (b'f', 4),
                    _ => unreachable!(),
                };
                let full_ndigits = width.div_ceil(radix_bits);
                let mut tmp_value = vec![b'0'; full_ndigits];
                let mut tmp_is_xz = vec![b'0'; full_ndigits];
                let full_width_nonzero =
                    NonZeroUsize::new(full_ndigits * radix_bits).unwrap();

                let mut pos = full_ndigits;
                for i in (0..hexint.len()).rev() {
                    let (d_value, d_is_xz) = match hexint[i] {
                        b'_' => continue,
                        b'x' => (b'0', radix_full_char),
                        b'z' => (radix_full_char, radix_full_char),
                        num => (num, b'0'),
                    };
                    pos -= 1;
                    tmp_value[pos] = d_value;
                    tmp_is_xz[pos] = d_is_xz;
                }
                if pos != 0 && tmp_is_xz[pos] == radix_full_char {
                    for i in 0..pos {
                        tmp_value[i] = tmp_value[pos];
                        tmp_is_xz[i] = tmp_is_xz[pos];
                    }
                }
                let value = ExtAwi::from_bytes_radix(
                    None,
                    &tmp_value,
                    radix,
                    full_width_nonzero,
                )
                .unwrap();
                let is_xz = ExtAwi::from_bytes_radix(
                    None,
                    &tmp_is_xz,
                    radix,
                    full_width_nonzero,
                )
                .unwrap();
                (value, is_xz)
            };

            let n_u128s = width.div_ceil(128);
            let mut ret = Vec::with_capacity(n_u128s);
            for i in 0..n_u128s {
                use std::num::Wrapping;
                let w = (width - i * 128).min(128);
                let mask = ((Wrapping(!0u128) << (128 - w)) >> (128 - w)).0;
                ret.push(WirexprBasic::Literal(
                    w,
                    value.to_u128() & mask,
                    is_xz.to_u128() & mask,
                ));
                if i + 1 != n_u128s {
                    value.lshr_(128).unwrap();
                    is_xz.lshr_(128).unwrap();
                }
            }
            ret.reverse();
            ret
        },
    )(i)
}

fn wirexpr(i: &[u8]) -> IResult<&[u8], Wirexpr> {
    use Wirexpr::*;
    use WirexprBasic::*;
    alt((
        // ident[int:int]
        map(
            pair(
                ws(ident),
                cut(opt(delimited(
                    char('['),
                    pair(ws(int), opt(preceded(char(':'), ws(int)))),
                    char(']'),
                ))),
            ),
            |(name, o)| {
                Basic(match o {
                    None => Full(name),
                    Some((l, None)) => SingleBit(name, l),
                    Some((l, Some(r))) => Slice(name, SVerilogRange(l, r)),
                })
            },
        ),
        // literal
        map(literal, |mut l| {
            if l.len() == 1 {
                Basic(l.swap_remove(0))
            } else {
                Concat(l)
            }
        }),
        // concat
        map(
            delimited(
                ws(char('{')),
                separated_list0(char(','), ws(cut(wirexpr))),
                ws(char('}')),
            ),
            |ws| {
                let mut v = Vec::new();
                for mut w in ws {
                    match w {
                        Basic(basic) => v.push(basic),
                        Concat(ref mut v2) => v.append(v2),
                    }
                }
                Concat(v)
            },
        ),
    ))(i)
}

fn portdef(i: &[u8]) -> IResult<&[u8], SVerilogPortDef> {
    use SVerilogPortDef::*;
    alt((
        map(ident, Basic),
        map(
            pair(
                preceded(ws(char('.')), cut(ident)),
                delimited(ws(char('(')), cut(wirexpr), ws(char(')'))),
            ),
            |(name, expr)| Conn(name, expr),
        ),
    ))(i)
}

fn wiredef_push_<'i>(
    i: &'i [u8],
    keep: bool,
    defs: &mut Vec<SVerilogWireDef>,
) -> IResult<&'i [u8], ()> {
    use WireDefType::*;
    let (i, typ) = ws(alt((
        value(Input, tag("input")),
        value(Output, tag("output")),
        value(InOut, tag("inout")),
        value(Wire, tag("wire")),
    )))(i)?;
    let (i, width) = opt(map(
        tuple((char('['), ws(int), char(':'), ws(int), char(']'))),
        |(_, l, _, r, _)| SVerilogRange(l, r),
    ))(i)?;
    let build_def = |name| SVerilogWireDef {
        name,
        width,
        typ,
        keep,
    };
    let (i, ()) = cut(map(ws(ident), |name| defs.push(build_def(name))))(i)?;
    let (i, ()) = fold_many0(
        preceded(char(','), ws(cut(ident))),
        || (),
        |_, name| defs.push(build_def(name)),
    )(i)?;
    let (i, _) = ws(cut(char(';')))(i)?;
    Ok((i, ()))
}

fn assign(i: &[u8]) -> IResult<&[u8], SVerilogAssign> {
    map(
        tuple((
            ws(tag("assign")),
            cut(wirexpr),
            ws(cut(char('='))),
            cut(wirexpr),
            ws(cut(char(';'))),
        )),
        |(_, lhs, _, rhs, _)| SVerilogAssign {
            lhs,
            rhs,
            keep: false,
        },
    )(i)
}

fn cell(i: &[u8]) -> IResult<&[u8], SVerilogCell> {
    map(
        tuple((
            ws(ident),
            ident,
            delimited(
                ws(char('(')),
                cut(separated_list0(
                    char(','),
                    map(
                        tuple((
                            ws(char('.')),
                            ident,
                            ws(char('(')),
                            opt(wirexpr),
                            ws(char(')')),
                        )),
                        |(_, name, _, expr, _)| expr.map(|e| (name, e)),
                    ),
                )),
                pair(ws(char(')')), ws(char(';'))),
            ),
        )),
        |(macro_name, cell_name, ioports)| SVerilogCell {
            macro_name,
            cell_name,
            ioports: ioports.into_iter().flatten().collect(),
            keep: false,
        },
    )(i)
}

fn module(i: &[u8]) -> IResult<&[u8], (CompactString, SVerilogModule)> {
    let (i, (name, ports)) = pair(
        preceded(ws(tag("module")), cut(ident)),
        cut(delimited(
            ws(char('(')),
            separated_list0(char(','), ws(portdef)),
            pair(ws(char(')')), ws(char(';'))),
        )),
    )(i)?;
    let mut defs = Vec::new();
    let mut assigns = Vec::new();
    let mut cells = Vec::new();

    let (i, _) = many0_count(|i| {
        let (i, keep) = leading_attrs(i)?;
        // Bind to local so the alt tuple (holding closures referencing `keep`)
        // is dropped before `keep` goes out of scope at end of the block.
        alt((
            |i| wiredef_push_(i, keep, &mut defs),
            |i| {
                let (i, mut a) = assign(i)?;
                a.keep = keep;
                assigns.push(a);
                Ok((i, ()))
            },
            |i| {
                let (i, mut c) = cell(i)?;
                c.keep = keep;
                cells.push(c);
                Ok((i, ()))
            },
        ))(i)
    })(i)?;
    let (i, _) = cut(ws(tag("endmodule")))(i)?;

    Ok((
        i,
        (
            name,
            SVerilogModule {
                ports,
                defs,
                assigns,
                cells,
                keep: false,
            },
        ),
    ))
}

fn sverilog(i: &[u8]) -> IResult<&[u8], SVerilog> {
    let (mut i, _) = many0_count(ws(char(';')))(i)?;
    let mut modules = Vec::new();
    loop {
        let (i2, keep) = leading_attrs(i)?;
        match module(i2) {
            Ok((i3, (name, mut module_data))) => {
                module_data.keep = keep;
                modules.push((name, module_data));
                let (i3, _) = many0_count(ws(char(';')))(i3)?;
                i = i3;
            }
            Err(_) => break,
        }
    }
    let (i, ()) = skip_whitespace_and_comment(i)?;
    Ok((i, SVerilog { modules }))
}

/// a `Display`able parsing error type, which prints at most
/// 50 characters after the error position.
pub(crate) struct ParseError {
    code: nom::error::ErrorKind,
    partial_input: String,
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for ParseError {
    fn from(e: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        match e {
            nom::Err::Incomplete(_) => ParseError {
                code: nom::error::ErrorKind::Fail,
                partial_input: "<incomplete>".into(),
            },
            nom::Err::Error(e) | nom::Err::Failure(e) => ParseError {
                code: e.code,
                partial_input: String::from_utf8_lossy(
                    &e.input[..e.input.len().min(50)],
                )
                .into_owned(),
            },
        }
    }
}

impl From<std::io::Error> for ParseError {
    fn from(e: std::io::Error) -> Self {
        ParseError {
            code: nom::error::ErrorKind::Fail,
            partial_input: format!("<io error: {}>", e),
        }
    }
}

impl From<ParseError> for String {
    fn from(e: ParseError) -> String {
        format!("{}", e)
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error {:?} at: {}", self.code, self.partial_input)
    }
}

pub(crate) fn parse_sverilog(i: &[u8]) -> Result<SVerilog, ParseError> {
    let (rem, sv) = sverilog(i)?;
    if rem.is_empty() {
        Ok(sv)
    } else {
        Err(nom::Err::Error(nom::error::Error {
            input: rem,
            code: nom::error::ErrorKind::Complete,
        })
        .into())
    }
}
