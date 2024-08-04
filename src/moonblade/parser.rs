// En tant que chef, je m'engage à ce que nous ne nous
// fassions pas *tous* tuer.
use pest::{
    iterators::{Pair, Pairs},
    pratt_parser::{Assoc, Op, PrattParser},
    Parser,
};
use pest_derive::Parser;

use super::functions::get_function;
use super::utils::downgrade_float;

#[derive(Parser)]
#[grammar = "moonblade/grammar.pest"]
pub struct MoonbladePestParser;

impl Rule {
    fn as_fn_op_str(&self) -> &'static str {
        match self {
            Self::num_eq => "__num_eq",
            Self::num_ne => "__num_ne",
            Self::num_lt => "__num_lt",
            Self::num_le => "__num_le",
            Self::num_gt => "__num_gt",
            Self::num_ge => "__num_ge",
            Self::str_eq => "eq",
            Self::str_ne => "ne",
            Self::str_lt => "lt",
            Self::str_le => "le",
            Self::str_gt => "gt",
            Self::str_ge => "ge",
            Self::add => "add",
            Self::sub => "sub",
            Self::mul => "mul",
            Self::div => "div",
            Self::idiv => "idiv",
            Self::pow => "pow",
            Self::rem => "mod",
            Self::concat => "concat",
            Self::and => "and",
            Self::or => "or",
            Self::not => "not",
            Self::neg => "neg",

            // NOTE: `pipe`, `in_op` and `not_in` are not covered by this match
            // because lhs and rhs are reversed.
            _ => unreachable!(),
        }
    }
}

lazy_static! {
    // NOTE: precedence taken from JavaScript and/or Python
    // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Operator_precedence#table
    // https://docs.python.org/3/reference/expressions.html

    // NOTE: indexing is supposed to be postfix, but I am cheating with
    // open_indexing here to pretend it's an infix operator.
    static ref PRATT_PARSER: PrattParser<Rule> = PrattParser::new()
        .op(Op::infix(Rule::pipe, Assoc::Left))
        .op(Op::infix(Rule::or, Assoc::Left))
        .op(Op::infix(Rule::and, Assoc::Left))
        .op(Op::infix(Rule::num_eq, Assoc::Left) |
            Op::infix(Rule::num_ne, Assoc::Left) |
            Op::infix(Rule::str_eq, Assoc::Left) |
            Op::infix(Rule::str_ne, Assoc::Left))
        .op(Op::infix(Rule::in_op, Assoc::Left) |
            Op::infix(Rule::not_in, Assoc::Left) |
            Op::infix(Rule::num_lt, Assoc::Left) |
            Op::infix(Rule::num_le, Assoc::Left) |
            Op::infix(Rule::num_gt, Assoc::Left) |
            Op::infix(Rule::num_ge, Assoc::Left) |
            Op::infix(Rule::str_lt, Assoc::Left) |
            Op::infix(Rule::str_le, Assoc::Left) |
            Op::infix(Rule::str_gt, Assoc::Left) |
            Op::infix(Rule::str_ge, Assoc::Left))
        .op(Op::infix(Rule::add, Assoc::Left) |
            Op::infix(Rule::sub, Assoc::Left) |
            Op::infix(Rule::concat, Assoc::Left))
        .op(Op::infix(Rule::mul, Assoc::Left) |
            Op::infix(Rule::div, Assoc::Left) |
            Op::infix(Rule::idiv, Assoc::Left) |
            Op::infix(Rule::rem, Assoc::Left))
        .op(Op::infix(Rule::pow, Assoc::Right))
        .op(Op::prefix(Rule::not) |
            Op::prefix(Rule::neg))
        .op(Op::infix(Rule::open_indexing, Assoc::Left));
}

fn parse_int(pair: Pair<Rule>) -> Result<i64, &str> {
    pair.as_str()
        .replace('_', "")
        .parse::<i64>()
        .or(Err("could not parse int"))
}

fn pratt_parse(pairs: Pairs<Rule>) -> Result<Expr, String> {
    PRATT_PARSER
        .map_primary(|primary| -> Result<Expr, String> {
            let expr = match primary.as_rule() {
                Rule::list => Expr::List(
                    primary
                        .into_inner()
                        .map(|t| pratt_parse(Pairs::single(t)))
                        .collect::<Result<_, _>>()?,
                ),
                Rule::map => {
                    // NOTE: we don't deduplicate keys
                    let mut map = Vec::new();

                    for entry in primary.into_inner() {
                        let mut sub = entry.into_inner();
                        let key_pair = sub.next().unwrap();
                        let key = match key_pair.as_rule() {
                            Rule::string => build_string(key_pair),
                            Rule::ident => key_pair.as_str().to_string(),
                            _ => unreachable!(),
                        };
                        let value = pratt_parse(Pairs::single(sub.next().unwrap()))?;

                        map.push((key, value));
                    }

                    Expr::Map(map)
                }
                Rule::int => Expr::Int(parse_int(primary)?),
                Rule::float => {
                    let n = primary
                        .as_str()
                        .replace('_', "")
                        .parse::<f64>()
                        .or(Err("could not parse float"))?;

                    Expr::Float(n)
                }
                Rule::full_slice => {
                    let mut pairs = primary.into_inner();
                    let start = parse_int(pairs.next().unwrap())?;
                    let end = parse_int(pairs.next().unwrap())?;

                    Expr::Slice(Slice::Full(start, end))
                }
                Rule::start_slice => {
                    let mut pairs = primary.into_inner();
                    let start = parse_int(pairs.next().unwrap())?;

                    Expr::Slice(Slice::Start(start))
                }
                Rule::end_slice => {
                    let mut pairs = primary.into_inner();
                    let end = parse_int(pairs.next().unwrap())?;

                    Expr::Slice(Slice::End(end))
                }
                Rule::string => Expr::Str(build_string(primary)),
                Rule::regex => {
                    let case_insensitive =
                        primary.clone().into_inner().any(|t| match t.as_rule() {
                            Rule::regex_flag => primary.as_str().contains('i'),
                            _ => false,
                        });

                    Expr::Regex(build_string(primary), case_insensitive)
                }
                Rule::underscore => Expr::Underscore,
                Rule::ident => Expr::Identifier(primary.as_str().to_string()),
                Rule::true_lit => Expr::Bool(true),
                Rule::false_lit => Expr::Bool(false),
                Rule::null => Expr::Null,
                Rule::expr => pratt_parse(primary.into_inner())?,
                Rule::func => {
                    let mut pairs = primary.into_inner();
                    let func_name = pairs.next().unwrap().as_str().to_lowercase();

                    Expr::Func(FunctionCall::new(
                        &func_name,
                        pairs
                            .map(|t| pratt_parse(Pairs::single(t)))
                            .collect::<Result<_, _>>()?,
                    ))
                }
                _ => unreachable!(),
            };

            Ok(expr)
        })
        .map_prefix(|op, rhs| {
            Ok(Expr::Func(FunctionCall::new(
                op.as_rule().as_fn_op_str(),
                vec![rhs?],
            )))
        })
        .map_infix(|lhs, op, rhs| {
            Ok(match op.as_rule() {
                // Swapping operands
                Rule::in_op => Expr::Func(FunctionCall::new("contains", vec![rhs?, lhs?])),
                Rule::not_in => Expr::Func(FunctionCall::new(
                    "not",
                    vec![Expr::Func(FunctionCall::new("contains", vec![rhs?, lhs?]))],
                )),

                // Indexing & slicing
                Rule::open_indexing => match rhs? {
                    Expr::Slice(slice) => Expr::Func(FunctionCall::new(
                        "slice",
                        match slice {
                            Slice::Full(start, end) => vec![lhs?, Expr::Int(start), Expr::Int(end)],
                            Slice::Start(start) => vec![lhs?, Expr::Int(start)],
                            Slice::End(end) => vec![lhs?, Expr::Int(0), Expr::Int(end)],
                        },
                    )),
                    rhs_res => Expr::Func(FunctionCall::new("get", vec![lhs?, rhs_res])),
                },

                // Pipe threading
                Rule::pipe => match rhs? {
                    Expr::Func(mut call) => {
                        call.fill_underscore(&lhs?);
                        Expr::Func(call)
                    }
                    Expr::Identifier(name) => match get_function(&name) {
                        None => Expr::Identifier(name),
                        Some(_) => Expr::Func(FunctionCall::new(&name, vec![lhs?])),
                    },
                    rest => rest,
                },

                // General case
                rule => Expr::Func(FunctionCall::new(rule.as_fn_op_str(), vec![lhs?, rhs?])),
            })
        })
        .parse(pairs)
        .map(|mut expr| {
            expr.simplify();
            expr
        })
}

fn build_string(pair: Pair<Rule>) -> String {
    let mut string = String::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::raw_double_quoted_string
            | Rule::raw_single_quoted_string
            | Rule::raw_regex_string => {
                string.push_str(inner.as_str());
            }
            Rule::regex_flag => break,
            Rule::escape => {
                let inner = inner.into_inner().next().unwrap();

                match inner.as_rule() {
                    Rule::predefined => {
                        string.push(match inner.as_str() {
                            "n" => '\n',
                            "r" => '\r',
                            "t" => '\t',
                            "\\" => '\\',
                            "\"" => '"',
                            "'" => '\'',
                            _ => unreachable!(),
                        });
                    }
                    _ => unreachable!(),
                }
            }
            Rule::escape_regex => {
                string.push_str(match inner.as_str() {
                    r"\n" => "\n",
                    r"\r" => "\r",
                    r"\t" => "\t",
                    r"\\" => "\\",
                    r"\/" => "/",
                    rest => rest,
                });
            }
            _ => {
                dbg!(inner);
                unreachable!()
            }
        }
    }

    string
}

#[derive(Debug, PartialEq, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<Expr>,
}

impl FunctionCall {
    fn new(name: &str, args: Vec<Expr>) -> Self {
        Self {
            name: name.to_string(),
            args,
        }
    }

    fn fill_underscore(&mut self, with: &Expr) {
        for arg in self.args.iter_mut() {
            match arg {
                Expr::Func(sub) => {
                    sub.fill_underscore(with);
                }
                Expr::Underscore => {
                    *arg = with.clone();
                }
                _ => (),
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Slice {
    Full(i64, i64),
    Start(i64),
    End(i64),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Func(FunctionCall),
    Int(i64),
    Float(f64),
    Identifier(String),
    Str(String),
    List(Vec<Expr>),
    Map(Vec<(String, Expr)>),
    Regex(String, bool),
    Slice(Slice),
    Bool(bool),
    Underscore,
    Null,
}

impl Expr {
    pub fn simplify(&mut self) {
        if let Self::Func(call) = self {
            if call.name == "neg" && call.args.len() == 1 {
                match call.args[0] {
                    Self::Int(n) => *self = Self::Int(-n),
                    Self::Float(n) => *self = Self::Float(-n),
                    _ => (),
                }
            } else {
                for arg in call.args.iter_mut() {
                    arg.simplify();
                }
            }
        }
    }

    pub fn try_to_usize(&self) -> Option<usize> {
        match self {
            Self::Int(n) => {
                if *n < 0 {
                    None
                } else {
                    Some(*n as usize)
                }
            }
            Self::Float(f) => match downgrade_float(*f) {
                None => None,
                Some(n) => {
                    if n < 0 {
                        None
                    } else {
                        Some(n as usize)
                    }
                }
            },
            _ => None,
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum ParseError {
    Pest(Box<pest::error::Error<Rule>>),
    Custom(String),
}

impl From<pest::error::Error<Rule>> for ParseError {
    fn from(value: pest::error::Error<Rule>) -> Self {
        Self::Pest(Box::new(value))
    }
}

impl From<String> for ParseError {
    fn from(value: String) -> Self {
        Self::Custom(value)
    }
}

pub fn parse_expression(input: &str) -> Result<Expr, ParseError> {
    let mut pairs = MoonbladePestParser::parse(Rule::full_expr, input)?;

    let first_pair = pairs.next().unwrap();

    Ok(pratt_parse(Pairs::single(first_pair))?)
}

pub fn parse_named_expressions(input: &str) -> Result<Vec<(Expr, String)>, ParseError> {
    let pairs = MoonbladePestParser::parse(Rule::named_exprs, input)?;

    pairs
        .filter(|p| !matches!(p.as_rule(), Rule::EOI))
        .map(|p| {
            let (name, p) = match p.as_rule() {
                Rule::expr => (p.as_span().as_str().to_string(), p),
                Rule::named_expr => {
                    let mut inner = p.into_inner();

                    debug_assert!(inner.len() == 2);

                    let expr = inner.next().unwrap();

                    let expr_name = inner.next().unwrap();
                    debug_assert!(matches!(expr_name.as_rule(), Rule::expr_name));

                    let expr_name_inner = expr_name.into_inner().next().unwrap();

                    let name = match expr_name_inner.as_rule() {
                        Rule::ident => expr_name_inner.as_str().to_string(),
                        Rule::string => build_string(expr_name_inner),
                        _ => unreachable!(),
                    };

                    (name, expr)
                }
                _ => unreachable!(),
            };

            let expr = pratt_parse(Pairs::single(p))?;

            Ok((expr, name))
        })
        .collect()
}

#[derive(Debug, PartialEq)]
pub struct Aggregation {
    pub agg_name: String,
    pub args: Vec<Expr>,
    pub func_name: String,
    pub expr_key: String,
}

pub type Aggregations = Vec<Aggregation>;

pub fn parse_aggregations(input: &str) -> Result<Aggregations, ParseError> {
    let pairs = MoonbladePestParser::parse(Rule::named_aggs, input)?;

    pairs
        .filter(|p| !matches!(p.as_rule(), Rule::EOI))
        .map(|p| {
            let (agg_name, expr_key, p) = match p.as_rule() {
                Rule::func => (
                    p.as_span().as_str().to_string(),
                    p.clone()
                        .into_inner()
                        .skip(1)
                        .take(1)
                        .map(|s| s.as_span().as_str())
                        .collect::<String>(),
                    p,
                ),
                Rule::named_func => {
                    let mut inner = p.into_inner();

                    debug_assert!(inner.len() == 2);

                    let func = inner.next().unwrap();

                    let expr_name = inner.next().unwrap();
                    debug_assert!(matches!(expr_name.as_rule(), Rule::expr_name));

                    let expr_name_inner = expr_name.into_inner().next().unwrap();

                    let name = match expr_name_inner.as_rule() {
                        Rule::ident => expr_name_inner.as_str().to_string(),
                        Rule::string => build_string(expr_name_inner),
                        _ => unreachable!(),
                    };

                    debug_assert!(matches!(func.as_rule(), Rule::func));

                    (
                        name,
                        func.clone()
                            .into_inner()
                            .skip(1)
                            .take(1)
                            .map(|s| s.as_span().as_str())
                            .collect::<String>(),
                        func,
                    )
                }
                _ => unreachable!(),
            };

            let expr = pratt_parse(Pairs::single(p))?;

            match expr {
                Expr::Func(call) => Ok(Aggregation {
                    agg_name,
                    args: call.args,
                    func_name: call.name,
                    expr_key,
                }),
                _ => unreachable!(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::Expr::*;
    use super::*;

    fn id(name: &str) -> Expr {
        Identifier(name.to_string())
    }

    fn func(name: &str, args: Vec<Expr>) -> Expr {
        Func(FunctionCall::new(name, args))
    }

    fn s(string: &str) -> Expr {
        Str(string.to_string())
    }

    fn r(string: &str) -> Expr {
        Regex(string.to_string(), false)
    }

    fn ri(string: &str) -> Expr {
        Regex(string.to_string(), true)
    }

    macro_rules! list {
        ( $( $x:expr ),* ) => {
            {
                let mut v = Vec::new();
                $(
                    v.push($x);
                )*
                List(v)
            }
        };
    }

    macro_rules! map {
        ( $( ($k:expr, $v:expr) ),* ) => {
            {
                let mut m = Vec::<(String, Expr)>::new();
                $(
                    m.push(($k.to_string(), $v));
                )*
                Map(m)
            }
        };
    }

    #[test]
    fn test_booleans() {
        assert_eq!(parse_expression("true"), Ok(Bool(true)));
        assert_eq!(parse_expression("false"), Ok(Bool(false)));
    }

    #[test]
    fn test_null() {
        assert_eq!(parse_expression("null"), Ok(Null));
    }

    #[test]
    fn test_integers() {
        assert_eq!(parse_expression("1"), Ok(Int(1)));
        assert_eq!(parse_expression("-45"), Ok(Int(-45)));
        assert_eq!(parse_expression("1_000"), Ok(Int(1000)));
    }

    #[test]
    fn test_floats() {
        assert_eq!(parse_expression("1.0"), Ok(Float(1.0)));
        assert_eq!(parse_expression("-45.5"), Ok(Float(-45.5)));
        assert_eq!(parse_expression("67.36"), Ok(Float(67.36)));
    }

    #[test]
    fn test_identifiers() {
        assert_eq!(parse_expression("name"), Ok(id("name")));
    }

    #[test]
    fn test_strings() {
        assert_eq!(parse_expression("\"test\""), Ok(s("test")));
        assert_eq!(parse_expression("'test'"), Ok(s("test")));
        assert_eq!(parse_expression("'  te st  '"), Ok(s("  te st  ")));
        assert_eq!(parse_expression("\"\\\"test\\\"\""), Ok(s("\"test\"")));
        assert_eq!(parse_expression("'\\'test\\''"), Ok(s("'test'")));
        assert_eq!(parse_expression(r"'\n\r\t\\\''"), Ok(s("\n\r\t\\'")));
        assert_eq!(parse_expression(r#""\n\r\t\\\"""#), Ok(s("\n\r\t\\\"")));
    }

    #[test]
    fn test_regexes() {
        assert_eq!(parse_expression("/test/"), Ok(r("test")));
        assert_eq!(parse_expression("/test/i"), Ok(ri("test")));
        assert_eq!(parse_expression("/tes.t?/"), Ok(r("tes.t?")));
        assert_eq!(parse_expression(r#"/te\.st/"#), Ok(r("te\\.st")));
        assert_eq!(parse_expression(r#"/te\/st/"#), Ok(r("te/st")));
        assert_eq!(parse_expression(r#"/te\nst/"#), Ok(r("te\nst")));
    }

    #[test]
    fn test_functions() {
        assert_eq!(
            parse_expression("add(count, 1)"),
            Ok(func("add", vec![id("count"), Int(1)]))
        );
    }

    #[test]
    fn test_infix() {
        assert_eq!(
            parse_expression("1 + 2"),
            Ok(func("add", vec![Int(1), Int(2)]))
        );
    }

    #[test]
    fn test_infix_associativity() {
        assert_eq!(
            parse_expression("1 + 2 * 4"),
            Ok(func("add", vec![Int(1), func("mul", vec![Int(2), Int(4)])]))
        );

        assert_eq!(
            parse_expression("2 ** 4"),
            Ok(func("pow", vec![Int(2), Int(4)]))
        )
    }

    #[test]
    fn test_expr_recursivity() {
        assert_eq!(
            parse_expression("1 + add(name, 3 * 4)"),
            Ok(func(
                "add",
                vec![
                    Int(1),
                    func("add", vec![id("name"), func("mul", vec![Int(3), Int(4)])])
                ]
            ))
        );
    }

    #[test]
    fn test_prefix_operators() {
        assert_eq!(parse_expression("-name"), Ok(func("neg", vec![id("name")])));
        assert_eq!(parse_expression("!45"), Ok(func("not", vec![Int(45)])));
        assert_eq!(
            parse_expression("!add(1, 2) + 4"),
            Ok(func(
                "add",
                vec![func("not", vec![func("add", vec![Int(1), Int(2)])]), Int(4)]
            ))
        );
        assert_eq!(
            parse_expression("!(add(1, 2) + 4)"),
            Ok(func(
                "not",
                vec![func("add", vec![func("add", vec![Int(1), Int(2)]), Int(4)])]
            ))
        );
    }

    #[test]
    fn test_containers() {
        assert_eq!(
            parse_expression("[1, 2, 3]"),
            Ok(list![Int(1), Int(2), Int(3)])
        );

        assert_eq!(
            parse_expression("[1, [2.5, 'test'], 3]"),
            Ok(list![Int(1), list![Float(2.5), s("test")], Int(3)])
        );

        assert_eq!(
            parse_expression("{one: 1, 'two': '5'}"),
            Ok(map![("one", Int(1)), ("two", s("5"))])
        );

        assert_eq!(
            parse_expression("{leaf: 1, nested: [2, {other_leaf: 3}]}"),
            Ok(map![
                ("leaf", Int(1)),
                ("nested", list![Int(2), map!(("other_leaf", Int(3)))])
            ])
        );
    }

    #[test]
    fn test_pipeline_operator() {
        // Basics
        assert_eq!(
            parse_expression("trim(count) | len(_)"),
            Ok(func("len", vec![func("trim", vec![id("count")])]))
        );

        assert_eq!(
            parse_expression("count + 1 | len(_)"),
            Ok(func("len", vec![func("add", vec![id("count"), Int(1)])]))
        );

        assert_eq!(
            parse_expression("add(trim(name) | len, 2)"),
            Ok(func(
                "add",
                vec![func("len", vec![func("trim", vec![id("name")])]), Int(2)]
            ))
        );

        // Double underscore
        assert_eq!(
            parse_expression("count | add(_, _)"),
            Ok(func("add", vec![id("count"), id("count")]))
        );

        // Nested underscores
        assert_eq!(
            parse_expression("count | add(1, sub(2, _))"),
            Ok(func(
                "add",
                vec![Int(1), func("sub", vec![Int(2), id("count")])]
            ))
        );

        assert_eq!(
            parse_expression("count | add(1, sub(2, _)) | len(_)"),
            Ok(func(
                "len",
                vec![func(
                    "add",
                    vec![Int(1), func("sub", vec![Int(2), id("count")])]
                )]
            ))
        );

        // Elision
        assert_eq!(
            parse_expression("trim(count) | len"),
            Ok(func("len", vec![func("trim", vec![id("count")])]))
        );

        // Trimming
        assert_eq!(
            parse_expression("trim(a) | add(a, b) | trim | add(a, b) | len"),
            Ok(func("len", vec![func("add", vec![id("a"), id("b")])]))
        );
    }

    #[test]
    fn test_aggregations() {
        assert_eq!(
            parse_aggregations("count(add(A, B) + 1)"),
            Ok(vec![Aggregation {
                agg_name: "count(add(A, B) + 1)".to_string(),
                func_name: "count".to_string(),
                expr_key: "add(A, B) + 1".to_string(),
                args: vec![func(
                    "add",
                    vec![func("add", vec![id("A"), id("B")]), Int(1)]
                ),]
            }])
        );

        assert_eq!(
            parse_aggregations("join(name, '|')"),
            Ok(vec![Aggregation {
                agg_name: "join(name, '|')".to_string(),
                func_name: "join".to_string(),
                expr_key: "name".to_string(),
                args: vec![id("name"), s("|")]
            }])
        );

        assert_eq!(
            parse_aggregations("count(a) as c, sum(b) as \"Sum\""),
            Ok(vec![
                Aggregation {
                    agg_name: "c".to_string(),
                    func_name: "count".to_string(),
                    expr_key: "a".to_string(),
                    args: vec![id("a")]
                },
                Aggregation {
                    agg_name: "Sum".to_string(),
                    func_name: "sum".to_string(),
                    expr_key: "b".to_string(),
                    args: vec![id("b")]
                }
            ])
        );
    }

    #[test]
    fn test_named_expressions() {
        assert_eq!(
            parse_named_expressions("name, 1 + 2 as three"),
            Ok(vec![
                (id("name"), "name".to_string()),
                (func("add", vec![Int(1), Int(2)]), "three".to_string())
            ])
        );
    }
}
