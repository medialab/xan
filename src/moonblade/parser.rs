// En tant que chef, je m'engage à ce que nous ne nous
// fassions pas *tous* tuer.
use lazy_static::lazy_static;
use pest::{
    iterators::{Pair, Pairs},
    pratt_parser::{Assoc, Op, PrattParser},
    Parser,
};
use pest_derive::Parser;

use super::functions::get_function;
use super::types::DynamicValue;
use super::utils::downgrade_float;

#[derive(Parser)]
#[grammar = "moonblade/grammar.pest"]
pub struct MoonbladePestParser;

impl Rule {
    fn as_fn_op_str(&self) -> &'static str {
        match self {
            Self::gen_eq => "==",
            Self::gen_ne => "!=",
            Self::gen_lt => "<",
            Self::gen_le => "<=",
            Self::gen_gt => ">",
            Self::gen_ge => ">=",
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
            // NOTE: and & or operators need to be resolved using if statements
            // Self::and => "and",
            // Self::or => "or",
            Self::not => "not",
            Self::neg => "neg",

            // NOTE: `point`, `pipe`, `in_op` and `not_in` are not covered by this match
            // because they are not as straightforward to implement.
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
        .op(Op::infix(Rule::gen_eq, Assoc::Left) |
            Op::infix(Rule::gen_ne, Assoc::Left) |
            Op::infix(Rule::str_eq, Assoc::Left) |
            Op::infix(Rule::str_ne, Assoc::Left))
        .op(Op::infix(Rule::in_op, Assoc::Left) |
            Op::infix(Rule::not_in, Assoc::Left) |
            Op::infix(Rule::gen_lt, Assoc::Left) |
            Op::infix(Rule::gen_le, Assoc::Left) |
            Op::infix(Rule::gen_gt, Assoc::Left) |
            Op::infix(Rule::gen_ge, Assoc::Left) |
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
        .op(Op::infix(Rule::open_indexing, Assoc::Left) |
            Op::infix(Rule::point, Assoc::Left));
}

fn parse_int(pair: Pair<Rule>) -> Result<i64, &str> {
    pair.as_str()
        .replace('_', "")
        .parse::<i64>()
        .or(Err("could not parse int"))
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
            _ => unreachable!(),
        }
    }

    string
}

fn build_function_argument(pair: Pair<Rule>) -> (Option<String>, Pair<Rule>) {
    match pair.as_rule() {
        Rule::func_arg => {
            let mut inner = pair.into_inner();
            let first = inner.next().unwrap();

            match first.as_rule() {
                Rule::ident => (Some(first.as_str().to_string()), inner.next().unwrap()),
                _ => (None, first),
            }
        }
        _ => unreachable!(),
    }
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
                Rule::star_slice_int => Expr::Int(parse_int(primary)?),
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

                    Expr::Slice(Slice::Closed(start, end))
                }
                Rule::start_slice => {
                    let mut pairs = primary.into_inner();
                    let start = parse_int(pairs.next().unwrap())?;

                    Expr::Slice(Slice::From(start))
                }
                Rule::end_slice => {
                    let mut pairs = primary.into_inner();
                    let end = parse_int(pairs.next().unwrap())?;

                    Expr::Slice(Slice::To(end))
                }
                Rule::string => Expr::Str(build_string(primary)),
                Rule::binary_string => Expr::BStr(build_string(primary).into_bytes()),
                Rule::regex => {
                    let case_insensitive =
                        primary.clone().into_inner().any(|t| match t.as_rule() {
                            Rule::regex_flag => primary.as_str().contains('i'),
                            _ => false,
                        });

                    Expr::Regex(build_string(primary), case_insensitive)
                }
                Rule::underscore => Expr::Underscore,
                Rule::ident => Expr::Identifier(
                    primary.as_str().trim_end_matches('?').to_string(),
                    primary.as_str().ends_with('?'),
                ),
                Rule::true_lit => Expr::Bool(true),
                Rule::false_lit => Expr::Bool(false),
                Rule::null => Expr::Null,
                Rule::expr => pratt_parse(primary.into_inner())?,
                Rule::lambda => {
                    let mut pairs = primary.into_inner();
                    let last_pair = pairs.next_back().unwrap();

                    debug_assert!(matches!(last_pair.as_rule(), Rule::expr));

                    let names = pairs
                        .take_while(|p| matches!(p.as_rule(), Rule::ident))
                        .map(|p| p.as_str().to_string())
                        .collect::<Vec<_>>();

                    let mut inner_expr = pratt_parse(last_pair.into_inner())?;
                    inner_expr.bind_lambda_args(&names);

                    Expr::Lambda(names, Box::new(inner_expr))
                }
                Rule::func => {
                    let mut pairs = primary.into_inner();
                    let func_name = pairs.next().unwrap().as_str().to_lowercase();

                    Expr::Func(FunctionCall::with_named_args(
                        &func_name,
                        pairs
                            .map(|p| build_function_argument(p))
                            .map(|(name, p)| pratt_parse(Pairs::single(p)).map(|r| (name, r)))
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
                            Slice::Closed(start, end) => {
                                vec![lhs?, Expr::Int(start), Expr::Int(end)]
                            }
                            Slice::From(start) => vec![lhs?, Expr::Int(start)],
                            Slice::To(end) => vec![lhs?, Expr::Int(0), Expr::Int(end)],
                            Slice::Full => unreachable!(),
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
                    Expr::Identifier(name, unsure) => match get_function(&name) {
                        None => Expr::Identifier(name, unsure),
                        Some(_) => Expr::Func(FunctionCall::new(&name, vec![lhs?])),
                    },
                    rest => rest,
                },

                // Short-circuiting and
                Rule::and => {
                    let lhs_res = lhs?;

                    // a && b => if(a, b, a)
                    Expr::Func(FunctionCall::new(
                        "if",
                        vec![lhs_res.clone(), rhs?, lhs_res],
                    ))
                }

                // Short-circuiting or
                Rule::or => {
                    let lhs_res = lhs?;

                    // a || b => if(a, a, b)
                    Expr::Func(FunctionCall::new(
                        "if",
                        vec![lhs_res.clone(), lhs_res, rhs?],
                    ))
                }

                // Access & call
                Rule::point => match rhs? {
                    Expr::Identifier(identifier, _) => {
                        Expr::Func(FunctionCall::new("get", vec![lhs?, Expr::Str(identifier)]))
                    }
                    Expr::Bool(value) => Expr::Func(FunctionCall::new(
                        "get",
                        vec![lhs?, Expr::Str(value.to_string())],
                    )),
                    Expr::Int(value) => {
                        Expr::Func(FunctionCall::new("get", vec![lhs?, Expr::Int(value)]))
                    }
                    Expr::Str(name) => {
                        Expr::Func(FunctionCall::new("get", vec![lhs?, Expr::Str(name)]))
                    }
                    Expr::BStr(name) => {
                        Expr::Func(FunctionCall::new("get", vec![lhs?, Expr::BStr(name)]))
                    }
                    Expr::Func(mut func_call) => {
                        func_call.args.insert(0, (None, lhs?));
                        Expr::Func(func_call)
                    }
                    _ => return Err("illegal access or call!".to_string()),
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

#[derive(Debug, PartialEq, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<(Option<String>, Expr)>,
}

impl FunctionCall {
    fn new(name: &str, args: Vec<Expr>) -> Self {
        Self {
            name: name.to_string(),
            args: args.into_iter().map(|arg| (None, arg)).collect(),
        }
    }

    fn with_named_args(name: &str, args: Vec<(Option<String>, Expr)>) -> Self {
        Self {
            name: name.to_string(),
            args,
        }
    }

    pub fn raw_args_as_ref(&self) -> Vec<&Expr> {
        self.args.iter().map(|(_, arg)| arg).collect()
    }

    fn fill_underscore(&mut self, with: &Expr) {
        for (_, arg) in self.args.iter_mut() {
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
pub enum Slice<T> {
    Full,
    Closed(T, T),
    From(T),
    To(T),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Func(FunctionCall),
    Lambda(Vec<String>, Box<Expr>),
    LambdaBinding(String),
    Int(i64),
    Float(f64),
    Identifier(String, bool),
    Str(String),
    BStr(Vec<u8>),
    List(Vec<Expr>),
    Map(Vec<(String, Expr)>),
    Regex(String, bool),
    Slice(Slice<i64>),
    StarSlice(Slice<DynamicValue>),
    Bool(bool),
    Underscore,
    Null,
}

impl Expr {
    pub fn bind_lambda_args(&mut self, names: &Vec<String>) {
        match self {
            Self::Identifier(name, _) => {
                if names.iter().any(|n| n == name) {
                    *self = Self::LambdaBinding(name.to_string());
                }
            }
            Self::Func(call) => {
                for (_, arg) in call.args.iter_mut() {
                    arg.bind_lambda_args(names);
                }
            }
            Self::List(exprs) => {
                for expr in exprs.iter_mut() {
                    expr.bind_lambda_args(names);
                }
            }
            Self::Map(exprs) => {
                for (_, expr) in exprs.iter_mut() {
                    expr.bind_lambda_args(names);
                }
            }
            _ => (),
        };
    }

    pub fn simplify(&mut self) {
        match self {
            Self::Func(call) => {
                if call.name == "neg" && call.args.len() == 1 {
                    match call.args[0].1 {
                        Self::Int(n) => *self = Self::Int(-n),
                        Self::Float(n) => *self = Self::Float(-n),
                        _ => (),
                    }
                } else {
                    for (_, arg) in call.args.iter_mut() {
                        arg.simplify();
                    }
                }
            }
            Self::List(exprs) => {
                for expr in exprs.iter_mut() {
                    expr.simplify();
                }
            }
            Self::Map(exprs) => {
                for (_, expr) in exprs.iter_mut() {
                    expr.simplify();
                }
            }
            _ => (),
        };
    }

    pub fn try_to_isize(&self) -> Option<isize> {
        match self {
            Self::Int(n) => Some(*n as isize),
            Self::Float(f) => downgrade_float(*f).map(|n| n as isize),
            _ => None,
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

fn parse_expression_name(pair: Pair<Rule>) -> String {
    debug_assert!(matches!(pair.as_rule(), Rule::expr_name));
    let expr_name_inner = pair.into_inner().next().unwrap();

    match expr_name_inner.as_rule() {
        Rule::ident => expr_name_inner.as_str().to_string(),
        Rule::string => build_string(expr_name_inner),
        _ => unreachable!(),
    }
}

pub fn parse_named_expressions(input: &str) -> Result<Vec<(Expr, String)>, ParseError> {
    let pairs = MoonbladePestParser::parse(Rule::named_exprs, input)?;

    pairs
        .filter(|p| !matches!(p.as_rule(), Rule::EOI))
        .map(|p| {
            let (name, p) = if p.as_rule() == Rule::star_slice {
                let mut inner = p.into_inner();
                let dummy_name = "".to_string();

                if inner.len() == 0 {
                    return Ok((Expr::StarSlice(Slice::Full), dummy_name));
                } else if inner.len() == 1 {
                    let slice = inner.next().unwrap();
                    let start = slice.as_rule() == Rule::start_star_slice;

                    let expr = pratt_parse(Pairs::single(slice.into_inner().next().unwrap()))?;
                    let value = match expr {
                        Expr::Int(i) => DynamicValue::Integer(i),
                        Expr::Str(s) => DynamicValue::from(s),
                        _ => unreachable!(),
                    };

                    if start {
                        return Ok((Expr::StarSlice(Slice::From(value)), dummy_name));
                    }

                    return Ok((Expr::StarSlice(Slice::To(value)), dummy_name));
                } else {
                    let start = inner.next().unwrap();
                    let end = inner.next().unwrap();

                    let start_expr =
                        pratt_parse(Pairs::single(start.into_inner().next().unwrap()))?;
                    let end_expr = pratt_parse(Pairs::single(end.into_inner().next().unwrap()))?;

                    let start_value = match start_expr {
                        Expr::Int(i) => DynamicValue::Integer(i),
                        Expr::Str(s) => DynamicValue::from(s),
                        _ => unreachable!(),
                    };
                    let end_value = match end_expr {
                        Expr::Int(i) => DynamicValue::Integer(i),
                        Expr::Str(s) => DynamicValue::from(s),
                        _ => unreachable!(),
                    };

                    return Ok((
                        Expr::StarSlice(Slice::Closed(start_value, end_value)),
                        dummy_name,
                    ));
                }
            } else {
                match p.as_rule() {
                    Rule::expr => (p.as_span().as_str().to_string(), p),
                    Rule::named_expr => {
                        let mut inner = p.into_inner();

                        debug_assert!(inner.len() == 2);

                        let expr = inner.next().unwrap();

                        let expr_name = inner.next().unwrap();
                        let name = parse_expression_name(expr_name);

                        (name, expr)
                    }
                    _ => unreachable!(),
                }
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
}

pub type Aggregations = Vec<Aggregation>;

pub fn parse_aggregations(input: &str) -> Result<Aggregations, ParseError> {
    let pairs = MoonbladePestParser::parse(Rule::named_aggs, input)?;

    pairs
        .filter(|p| !matches!(p.as_rule(), Rule::EOI))
        .map(|p| {
            let (agg_name, p) = match p.as_rule() {
                Rule::func => (p.as_span().as_str().to_string(), p),
                Rule::named_func => {
                    let mut inner = p.into_inner();

                    debug_assert!(inner.len() == 2);

                    let func = inner.next().unwrap();

                    let expr_name = inner.next().unwrap();
                    let name = parse_expression_name(expr_name);

                    debug_assert!(matches!(func.as_rule(), Rule::func));

                    (name, func)
                }
                _ => unreachable!(),
            };

            let expr = pratt_parse(Pairs::single(p))?;

            match expr {
                Expr::Func(call) => Ok(Aggregation {
                    agg_name,
                    args: call.args.into_iter().map(|(_, arg)| arg).collect(),
                    func_name: call.name,
                }),
                _ => unreachable!(),
            }
        })
        .collect()
}

#[derive(Debug, PartialEq)]
pub struct ScrapingLeaf {
    pub name: String,
    pub expr: Expr,
    pub processing: Option<Expr>,
}

#[derive(Debug, PartialEq)]
pub enum ScrapingNode {
    Brackets(ScrapingBrackets),
    Leaf(ScrapingLeaf),
}

#[derive(Debug, PartialEq)]
pub struct ScrapingBrackets {
    pub selection_expr: Expr,
    pub nodes: Vec<ScrapingNode>,
}

fn parse_scraping_leaf(pair: Pair<Rule>) -> Result<ScrapingLeaf, ParseError> {
    debug_assert!(matches!(pair.as_rule(), Rule::scraping_leaf));

    let mut pairs = pair.into_inner().collect::<Vec<_>>();

    let (expr, processing) = if pairs.len() == 3 {
        let processing = pratt_parse(Pairs::single(pairs.pop().unwrap()))?;
        let expr = pratt_parse(Pairs::single(pairs.pop().unwrap()))?;

        (expr, Some(processing))
    } else {
        let expr = pratt_parse(Pairs::single(pairs.pop().unwrap()))?;

        (expr, None)
    };

    let name = parse_expression_name(pairs.pop().unwrap());

    Ok(ScrapingLeaf {
        name,
        expr,
        processing,
    })
}

fn parse_css_selector(pair: Pair<Rule>) -> Expr {
    let mut css = pair.as_str().trim().to_string();

    if css.starts_with("& ") {
        css = css.replacen('&', ":scope", 1);
    }

    Expr::Func(FunctionCall {
        name: "one".to_string(),
        args: vec![(None, Expr::Str(css))],
    })
}

fn parse_scraping_brackets(pair: Pair<Rule>) -> Result<ScrapingBrackets, ParseError> {
    debug_assert!(matches!(pair.as_rule(), Rule::scraping_brackets));

    let mut pairs = pair.into_inner();
    let selection = pairs.next().unwrap();

    let selection_expr = match selection.as_rule() {
        Rule::expr => {
            if !matches!(
                selection.clone().into_inner().next().unwrap().as_rule(),
                Rule::func
            ) {
                parse_css_selector(selection)
            } else {
                pratt_parse(Pairs::single(selection))?
            }
        }
        Rule::css_selector => parse_css_selector(selection),
        _ => unreachable!(),
    };

    let mut nodes = Vec::new();

    for pair in pairs {
        match pair.as_rule() {
            Rule::scraping_brackets => {
                nodes.push(ScrapingNode::Brackets(parse_scraping_brackets(pair)?));
            }
            Rule::scraping_leaf => {
                nodes.push(ScrapingNode::Leaf(parse_scraping_leaf(pair)?));
            }
            _ => unreachable!(),
        }
    }

    Ok(ScrapingBrackets {
        selection_expr,
        nodes,
    })
}

pub fn parse_scraper(input: &str) -> Result<Vec<ScrapingBrackets>, ParseError> {
    MoonbladePestParser::parse(Rule::scraping_expr, input)?
        .filter(|p| !matches!(p.as_rule(), Rule::EOI))
        .map(|p| parse_scraping_brackets(p))
        .collect::<Result<Vec<_>, _>>()
}

#[cfg(test)]
mod tests {
    use super::Expr::*;
    use super::*;

    fn id(name: &str) -> Expr {
        Identifier(name.to_string(), false)
    }

    fn unsure_id(name: &str) -> Expr {
        Identifier(name.to_string(), true)
    }

    fn func(name: &str, args: Vec<Expr>) -> Expr {
        Func(FunctionCall::new(name, args))
    }

    fn nfunc(name: &str, args: Vec<(Option<&str>, Expr)>) -> Expr {
        Func(FunctionCall::with_named_args(
            name,
            args.into_iter()
                .map(|(name, expr)| (name.map(|s| s.to_string()), expr))
                .collect(),
        ))
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

    fn lambda(names: Vec<&str>, expr: Expr) -> Expr {
        Lambda(
            names.into_iter().map(|s| s.to_string()).collect(),
            Box::new(expr),
        )
    }

    fn lb(string: &str) -> Expr {
        LambdaBinding(string.to_string())
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
        assert_eq!(parse_expression("name?"), Ok(unsure_id("name")));
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
    fn test_lambdas() {
        assert_eq!(
            parse_expression("map(array, x => x + 1)"),
            Ok(func(
                "map",
                vec![
                    id("array"),
                    lambda(vec!["x"], func("add", vec![lb("x"), Int(1)]))
                ]
            ))
        );

        assert_eq!(
            parse_expression("map(array, (x) => x + 1)"),
            Ok(func(
                "map",
                vec![
                    id("array"),
                    lambda(vec!["x"], func("add", vec![lb("x"), Int(1)]))
                ]
            ))
        );

        assert_eq!(
            parse_expression("map(array, (x, y) => x + y)"),
            Ok(func(
                "map",
                vec![
                    id("array"),
                    lambda(vec!["x", "y"], func("add", vec![lb("x"), lb("y")]))
                ]
            ))
        );

        assert_eq!(
            parse_expression("map(array, (x) => x + age)"),
            Ok(func(
                "map",
                vec![
                    id("array"),
                    lambda(vec!["x"], func("add", vec![lb("x"), id("age")]))
                ]
            ))
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
    fn test_named_arguments() {
        assert_eq!(
            parse_expression("add(45, plus=5)"),
            Ok(nfunc("add", vec![(None, Int(45)), (Some("plus"), Int(5))]))
        );
    }

    #[test]
    fn test_aggregations() {
        assert_eq!(
            parse_aggregations("count(add(A, B) + 1)"),
            Ok(vec![Aggregation {
                agg_name: "count(add(A, B) + 1)".to_string(),
                func_name: "count".to_string(),
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
                args: vec![id("name"), s("|")]
            }])
        );

        assert_eq!(
            parse_aggregations("count(a) as c, sum(b) as \"Sum\""),
            Ok(vec![
                Aggregation {
                    agg_name: "c".to_string(),
                    func_name: "count".to_string(),
                    args: vec![id("a")]
                },
                Aggregation {
                    agg_name: "Sum".to_string(),
                    func_name: "sum".to_string(),
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

    #[test]
    fn test_scraper() {
        let parsed = parse_scraper(
            "h2 > a {
                title: text, lower;
                url: attr('href');

                & > time {
                    date: attr('datetime');
                }

                * {
                    text: text, upper(value);
                }
            }
            all('p') {
                content: text;
            }
            one('div').all('.content') {
              info: text;
            }",
        );

        fn leaf(name: &str, getter: Expr) -> ScrapingNode {
            ScrapingNode::Leaf(ScrapingLeaf {
                name: name.to_string(),
                expr: getter,
                processing: None,
            })
        }

        fn leafp(name: &str, getter: Expr, processing: Expr) -> ScrapingNode {
            ScrapingNode::Leaf(ScrapingLeaf {
                name: name.to_string(),
                expr: getter,
                processing: Some(processing),
            })
        }

        fn brackets(func_name: &str, css: &str, nodes: Vec<ScrapingNode>) -> ScrapingBrackets {
            ScrapingBrackets {
                selection_expr: func(func_name, vec![s(css)]),
                nodes,
            }
        }

        assert_eq!(
            parsed,
            Ok(vec![
                brackets(
                    "one",
                    "h2 > a",
                    vec![
                        leafp("title", id("text"), id("lower")),
                        leaf("url", func("attr", vec![s("href")])),
                        ScrapingNode::Brackets(brackets(
                            "one",
                            ":scope > time",
                            vec![leaf("date", func("attr", vec![s("datetime")]))]
                        )),
                        ScrapingNode::Brackets(brackets(
                            "one",
                            "*",
                            vec![leafp("text", id("text"), func("upper", vec![id("value")]))]
                        ))
                    ]
                ),
                brackets("all", "p", vec![leaf("content", id("text"))]),
                ScrapingBrackets {
                    selection_expr: func("all", vec![func("one", vec![s("div")]), s(".content")]),
                    nodes: vec![leaf("info", id("text"))]
                }
            ])
        );
    }
}
