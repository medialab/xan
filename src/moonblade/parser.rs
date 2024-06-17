// En tant que chef, je m'engage à ce que nous ne nous
// fassions pas *tous* tuer.
use std::collections::BTreeMap;

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use pratt::{Affix, Associativity, PrattError, PrattParser, Precedence};

use super::functions::get_function;
use super::utils::downgrade_float;

#[derive(Parser)]
#[grammar = "moonblade/grammar.pest"]
pub struct MoonbladePestParser;

#[derive(Debug, PartialEq)]
enum Operator {
    NumEq,
    NumNe,
    NumLt,
    NumLe,
    NumGt,
    NumGe,
    StrEq,
    StrNe,
    StrLt,
    StrLe,
    StrGt,
    StrGe,
    Add,
    Sub,
    Mul,
    Div,
    IDiv,
    Mod,
    Pow,
    Concat,
    And,
    Or,
    In,
    NotIn,
    Not,
    Neg,
    Pipe,
}

impl Operator {
    fn as_fn_str(&self) -> &'static str {
        match self {
            Self::NumEq => "__num_eq",
            Self::NumNe => "__num_ne",
            Self::NumLt => "__num_lt",
            Self::NumLe => "__num_le",
            Self::NumGt => "__num_gt",
            Self::NumGe => "__num_ge",
            Self::StrEq => "eq",
            Self::StrNe => "ne",
            Self::StrLt => "lt",
            Self::StrLe => "le",
            Self::StrGt => "gt",
            Self::StrGe => "ge",
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::Div => "div",
            Self::IDiv => "idiv",
            Self::Pow => "pow",
            Self::Mod => "mod",
            Self::Concat => "concat",
            Self::And => "and",
            Self::Or => "or",
            Self::Not => "not",
            Self::Neg => "neg",

            // NOTE: `Pipe`, `In` and `NotIn` are not covered by this match
            // because lhs and rhs are reversed.
            _ => unreachable!(),
        }
    }

    fn to_fn_string(&self) -> String {
        self.as_fn_str().to_string()
    }

    // NOTE: precedence taken from JavaScript and/or Python
    // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Operator_precedence#table
    // https://docs.python.org/3/reference/expressions.html
    fn precedence(&self) -> Affix {
        match self {
            Self::Not | Self::Neg => Affix::Prefix(Precedence(14)),
            Self::Pow => Affix::Infix(Precedence(13), Associativity::Right),
            Self::Mul | Self::Div | Self::IDiv | Self::Mod => {
                Affix::Infix(Precedence(12), Associativity::Left)
            }
            Self::Add | Self::Sub | Self::Concat => {
                Affix::Infix(Precedence(11), Associativity::Left)
            }
            Self::In
            | Self::NotIn
            | Self::NumLt
            | Self::NumLe
            | Self::NumGt
            | Self::NumGe
            | Self::StrLt
            | Self::StrLe
            | Self::StrGt
            | Self::StrGe => Affix::Infix(Precedence(9), Associativity::Left),
            Self::NumEq | Self::NumNe | Self::StrEq | Self::StrNe => {
                Affix::Infix(Precedence(8), Associativity::Left)
            }
            Self::And => Affix::Infix(Precedence(4), Associativity::Left),
            Self::Or => Affix::Infix(Precedence(3), Associativity::Left),
            Self::Pipe => Affix::Infix(Precedence(2), Associativity::Left),
        }
    }
}

#[derive(Debug, PartialEq)]
enum TokenTree<'a> {
    Infix(Operator),
    Primary(Pair<'a, Rule>),
    Expr(Vec<TokenTree<'a>>),
    Func(String, Vec<TokenTree<'a>>),
}

impl<'a> From<Pair<'a, Rule>> for TokenTree<'a> {
    fn from(pair: Pair<'a, Rule>) -> Self {
        match pair.as_rule() {
            Rule::map
            | Rule::list
            | Rule::string
            | Rule::regex
            | Rule::int
            | Rule::float
            | Rule::ident
            | Rule::underscore
            | Rule::true_lit
            | Rule::false_lit
            | Rule::null => TokenTree::Primary(pair),

            Rule::num_eq => TokenTree::Infix(Operator::NumEq),
            Rule::num_ne => TokenTree::Infix(Operator::NumNe),
            Rule::num_lt => TokenTree::Infix(Operator::NumLt),
            Rule::num_le => TokenTree::Infix(Operator::NumLe),
            Rule::num_gt => TokenTree::Infix(Operator::NumGt),
            Rule::num_ge => TokenTree::Infix(Operator::NumGe),
            Rule::str_eq => TokenTree::Infix(Operator::StrEq),
            Rule::str_ne => TokenTree::Infix(Operator::StrNe),
            Rule::str_lt => TokenTree::Infix(Operator::StrLt),
            Rule::str_le => TokenTree::Infix(Operator::StrLe),
            Rule::str_gt => TokenTree::Infix(Operator::StrGt),
            Rule::str_ge => TokenTree::Infix(Operator::StrGe),
            Rule::add => TokenTree::Infix(Operator::Add),
            Rule::sub => TokenTree::Infix(Operator::Sub),
            Rule::mul => TokenTree::Infix(Operator::Mul),
            Rule::div => TokenTree::Infix(Operator::Div),
            Rule::idiv => TokenTree::Infix(Operator::IDiv),
            Rule::rem => TokenTree::Infix(Operator::Mod),
            Rule::pow => TokenTree::Infix(Operator::Pow),
            Rule::concat => TokenTree::Infix(Operator::Concat),
            Rule::and => TokenTree::Infix(Operator::And),
            Rule::or => TokenTree::Infix(Operator::Or),
            Rule::in_op => TokenTree::Infix(Operator::In),
            Rule::not_in => TokenTree::Infix(Operator::NotIn),
            Rule::pipe => TokenTree::Infix(Operator::Pipe),

            Rule::not => TokenTree::Infix(Operator::Not),
            Rule::neg => TokenTree::Infix(Operator::Neg),

            Rule::expr => {
                let mut pairs = pair.into_inner();

                if pairs.len() == 1 {
                    Self::from(pairs.next().unwrap())
                } else {
                    TokenTree::Expr(pairs.map(Self::from).collect())
                }
            }

            Rule::func => {
                let mut pairs = pair.into_inner();
                let func_name = pairs.next().unwrap().as_str().to_lowercase();

                TokenTree::Func(func_name, pairs.map(Self::from).collect())
            }

            _ => {
                unreachable!();
            }
        }
    }
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
pub enum Expr {
    Func(FunctionCall),
    Int(i64),
    Float(f64),
    Identifier(String),
    Str(String),
    List(Vec<Expr>),
    Map(BTreeMap<String, Expr>),
    Regex(String, bool),
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

struct MoonbladePrattParser;

impl<'a, I> PrattParser<I> for MoonbladePrattParser
where
    I: Iterator<Item = TokenTree<'a>>,
{
    type Error = String;
    type Input = TokenTree<'a>;
    type Output = Expr;

    fn query(&mut self, tree: &TokenTree) -> Result<Affix, Self::Error> {
        let affix = match tree {
            TokenTree::Infix(op) => op.precedence(),
            TokenTree::Expr(_) => Affix::Nilfix,
            TokenTree::Func(_, _) => Affix::Nilfix,
            TokenTree::Primary(_) => Affix::Nilfix,
        };

        Ok(affix)
    }

    fn primary(&mut self, tree: TokenTree) -> Result<Expr, Self::Error> {
        let expr = match tree {
            TokenTree::Primary(token) => match token.as_rule() {
                Rule::list => Expr::List(
                    token
                        .into_inner()
                        .map(|t| self.parse(&mut vec![TokenTree::from(t)].into_iter()))
                        .collect::<Result<_, _>>()
                        .map_err(|err| match err {
                            PrattError::UserError(inner) => inner,
                            _ => unreachable!(),
                        })?,
                ),
                Rule::map => {
                    let mut map = BTreeMap::new();

                    for entry in token.into_inner() {
                        let mut sub = entry.into_inner();
                        let key_pair = sub.next().unwrap();
                        let key = match key_pair.as_rule() {
                            Rule::string => build_string(key_pair),
                            Rule::ident => key_pair.as_str().to_string(),
                            _ => unreachable!(),
                        };
                        let value = self
                            .parse(&mut vec![TokenTree::from(sub.next().unwrap())].into_iter())
                            .map_err(|err| match err {
                                PrattError::UserError(inner) => inner,
                                _ => unreachable!(),
                            })?;

                        map.insert(key, value);
                    }

                    Expr::Map(map)
                }
                Rule::int => {
                    let n = token
                        .as_str()
                        .replace('_', "")
                        .parse::<i64>()
                        .or(Err("could not parse int"))?;

                    Expr::Int(n)
                }
                Rule::float => {
                    let n = token
                        .as_str()
                        .replace('_', "")
                        .parse::<f64>()
                        .or(Err("could not parse float"))?;

                    Expr::Float(n)
                }
                Rule::string => Expr::Str(build_string(token)),
                Rule::regex => {
                    let case_insensitive = token.clone().into_inner().any(|t| match t.as_rule() {
                        Rule::regex_flag => token.as_str().contains('i'),
                        _ => false,
                    });

                    Expr::Regex(build_string(token), case_insensitive)
                }
                Rule::underscore => Expr::Underscore,
                Rule::ident => Expr::Identifier(token.as_str().to_string()),
                Rule::true_lit => Expr::Bool(true),
                Rule::false_lit => Expr::Bool(false),
                Rule::null => Expr::Null,
                _ => unreachable!(),
            },
            TokenTree::Expr(group) => {
                self.parse(&mut group.into_iter())
                    .map_err(|err| match err {
                        PrattError::UserError(inner) => inner,
                        _ => unreachable!(),
                    })?
            }
            TokenTree::Func(name, group) => Expr::Func(FunctionCall {
                name,
                args: group
                    .into_iter()
                    .map(|g| self.parse(&mut vec![g].into_iter()))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| match err {
                        PrattError::UserError(inner) => inner,
                        _ => unreachable!(),
                    })?,
            }),
            _ => unreachable!(),
        };

        Ok(expr)
    }

    fn infix(&mut self, lhs: Expr, tree: TokenTree, rhs: Expr) -> Result<Expr, Self::Error> {
        Ok(match tree {
            TokenTree::Infix(op) => {
                match op {
                    // Swapping operands
                    Operator::In => Expr::Func(FunctionCall {
                        name: "contains".to_string(),
                        args: vec![rhs, lhs],
                    }),
                    Operator::NotIn => Expr::Func(FunctionCall {
                        name: "not".to_string(),
                        args: vec![Expr::Func(FunctionCall {
                            name: "contains".to_string(),
                            args: vec![rhs, lhs],
                        })],
                    }),

                    // Pipe threading
                    Operator::Pipe => match rhs {
                        Expr::Func(mut call) => {
                            call.fill_underscore(&lhs);
                            Expr::Func(call)
                        }
                        Expr::Identifier(name) => match get_function(&name) {
                            None => Expr::Identifier(name),
                            Some(_) => Expr::Func({
                                FunctionCall {
                                    name,
                                    args: vec![lhs],
                                }
                            }),
                        },
                        _ => rhs,
                    },

                    // General case
                    _ => Expr::Func(FunctionCall {
                        name: op.to_fn_string(),
                        args: vec![lhs, rhs],
                    }),
                }
            }
            _ => unreachable!(),
        })
    }

    fn prefix(&mut self, tree: TokenTree, rhs: Expr) -> Result<Expr, Self::Error> {
        let args = vec![rhs];

        Ok(match tree {
            TokenTree::Infix(op) => Expr::Func(FunctionCall {
                name: op.to_fn_string(),
                args,
            }),
            _ => unreachable!(),
        })
    }

    fn postfix(&mut self, _lhs: Expr, _tree: TokenTree) -> Result<Expr, Self::Error> {
        unreachable!()
    }
}

#[derive(PartialEq, Debug)]
pub enum ParseError {
    PestError(Box<pest::error::Error<Rule>>),
    PrattError(String),
}

impl ParseError {
    fn from_pest_error(error: pest::error::Error<Rule>) -> Self {
        Self::PestError(Box::new(error))
    }
}

fn apply_pratt_parser(token_tree: TokenTree) -> Result<Expr, ParseError> {
    match MoonbladePrattParser.parse(&mut vec![token_tree].into_iter()) {
        Err(err) => Err(ParseError::PrattError(err.to_string())),
        Ok(mut expr) => {
            expr.simplify();
            Ok(expr)
        }
    }
}

pub fn parse_expression(input: &str) -> Result<Expr, ParseError> {
    let mut pairs =
        MoonbladePestParser::parse(Rule::full_expr, input).map_err(ParseError::from_pest_error)?;

    let first_pair = pairs.next().unwrap();

    let token_tree = TokenTree::from(first_pair);

    apply_pratt_parser(token_tree)
}

pub fn parse_named_expressions(input: &str) -> Result<Vec<(Expr, String)>, ParseError> {
    let pairs = MoonbladePestParser::parse(Rule::named_exprs, input)
        .map_err(ParseError::from_pest_error)?;

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

            let token_tree = TokenTree::from(p);
            let expr = apply_pratt_parser(token_tree)?;

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
    let pairs =
        MoonbladePestParser::parse(Rule::named_aggs, input).map_err(ParseError::from_pest_error)?;

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

            let token_tree = TokenTree::from(p);

            let expr = apply_pratt_parser(token_tree)?;

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
        Func(FunctionCall {
            name: name.to_string(),
            args,
        })
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
                let mut m = BTreeMap::<String, Expr>::new();
                $(
                    m.insert($k.to_string(), $v);
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
