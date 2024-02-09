// En tant que chef, je m'engage à ce que nous ne nous
// fassions pas *tous* tuer.
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use pratt::{Affix, Associativity, PrattParser, Precedence};

use super::functions::get_function;
use super::utils::downgrade_float;

#[derive(Parser)]
#[grammar = "moonblade/grammar.pest"]
pub struct MoonbladePestParser;

#[derive(Debug, PartialEq)]
enum Operator {
    Add,
    Mul,
    Not,
}

impl Operator {
    fn as_fn_str(&self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Mul => "mul",
            Self::Not => "not",
        }
    }

    fn to_fn_string(&self) -> String {
        self.as_fn_str().to_string()
    }

    // NOTE: precdence taken from JavaScript
    // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Operator_precedence#table
    fn precedence(&self) -> Affix {
        match self {
            Self::Not => Affix::Prefix(Precedence(14)),
            Self::Mul => Affix::Infix(Precedence(12), Associativity::Left),
            Self::Add => Affix::Infix(Precedence(11), Associativity::Left),
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
            Rule::string
            | Rule::case_insensitive_regex
            | Rule::case_sensitive_regex
            | Rule::int
            | Rule::float
            | Rule::ident
            | Rule::special_ident
            | Rule::underscore
            | Rule::true_lit
            | Rule::false_lit
            | Rule::null => TokenTree::Primary(pair),
            Rule::add => TokenTree::Infix(Operator::Add),
            Rule::mul => TokenTree::Infix(Operator::Mul),
            Rule::not => TokenTree::Infix(Operator::Not),
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
                dbg!(&pair);
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
    fn has_underscore(&self) -> bool {
        self.args.iter().any(|arg| match arg {
            Expr::Func(sub_function_call) => sub_function_call.has_underscore(),
            Expr::Underscore => true,
            _ => false,
        })
    }

    fn count_underscores(&self) -> usize {
        self.args
            .iter()
            .map(|arg| match arg {
                Expr::Func(sub_function_call) => sub_function_call.count_underscores(),
                Expr::Underscore => 1,
                _ => 0,
            })
            .sum()
    }

    fn fill_underscore(&mut self, with: &Expr) {
        if let Expr::Func(_) = with {
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
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Func(FunctionCall),
    Int(i64),
    Float(f64),
    Identifier(String),
    SpecialIdentifier(String),
    Str(String),
    Regex(String, bool),
    Bool(bool),
    Underscore,
    Null,
}

impl Expr {
    pub fn has_underscore(&self) -> bool {
        match self {
            Self::Func(call) => call.has_underscore(),
            _ => false,
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
                Rule::case_insensitive_regex => Expr::Regex(build_string(token), true),
                Rule::case_sensitive_regex => Expr::Regex(build_string(token), false),
                Rule::underscore => Expr::Underscore,
                Rule::ident => Expr::Identifier(token.as_str().to_string()),
                Rule::special_ident => Expr::SpecialIdentifier(token.as_str()[1..].to_string()),
                Rule::true_lit => Expr::Bool(true),
                Rule::false_lit => Expr::Bool(false),
                Rule::null => Expr::Null,
                _ => unreachable!(),
            },
            TokenTree::Expr(group) => self.parse(&mut group.into_iter()).unwrap(),
            TokenTree::Func(name, group) => Expr::Func(FunctionCall {
                name,
                args: group
                    .into_iter()
                    .map(|g| self.parse(&mut vec![g].into_iter()).unwrap())
                    .collect(),
            }),
            _ => unreachable!(),
        };

        Ok(expr)
    }

    fn infix(&mut self, lhs: Expr, tree: TokenTree, rhs: Expr) -> Result<Expr, Self::Error> {
        let args = vec![lhs, rhs];

        Ok(match tree {
            TokenTree::Infix(op) => Expr::Func(FunctionCall {
                name: op.to_fn_string(),
                args,
            }),
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

#[cfg(test)]
fn parse_expression(input: &str) -> Result<Expr, ParseError> {
    let mut pairs =
        MoonbladePestParser::parse(Rule::full_expr, input).map_err(ParseError::from_pest_error)?;

    let first_pair = pairs.next().unwrap();

    let token_tree = TokenTree::from(first_pair);

    MoonbladePrattParser
        .parse(&mut vec![token_tree].into_iter())
        .map_err(|err| ParseError::PrattError(err.to_string()))
}

pub type Pipeline = Vec<Expr>;

// TODO: trim, unfurl

fn parse_pipeline(input: &str) -> Result<Pipeline, ParseError> {
    let mut pairs =
        MoonbladePestParser::parse(Rule::pipeline, input).map_err(ParseError::from_pest_error)?;

    let first_pair = pairs.next().unwrap();

    debug_assert!(matches!(first_pair.as_rule(), Rule::pipeline));

    first_pair
        .into_inner()
        .filter(|p| !matches!(p.as_rule(), Rule::EOI))
        .map(|p| {
            let token_tree = TokenTree::from(p);

            MoonbladePrattParser
                .parse(&mut vec![token_tree].into_iter())
                .map_err(|err| ParseError::PrattError(err.to_string()))
        })
        .collect()
}

fn handle_pipeline_elision(pipeline: Pipeline) -> Pipeline {
    pipeline
        .into_iter()
        .enumerate()
        .map(|(i, expr)| {
            if i == 0 {
                expr
            } else if let Expr::Identifier(ref name) = expr {
                match get_function(name) {
                    None => expr,
                    Some(_) => Expr::Func(FunctionCall {
                        name: name.to_string(),
                        args: vec![Expr::Underscore],
                    }),
                }
            } else {
                expr
            }
        })
        .collect()
}

// Example: trim(a) | add(a, b) | trim | add(a, b) | len -> add(a, b) | len
fn trim_pipeline(pipeline: Pipeline) -> Pipeline {
    match pipeline
        .iter()
        .enumerate()
        .rev()
        .find(|(i, arg)| *i != 0 && !arg.has_underscore())
        .map(|r| r.0)
    {
        None => pipeline,
        Some(index) => pipeline[index..].to_vec(),
    }
}

// Example: trim(a) | len | add(b, _) -> add(b, len(trim(a)))
// NOTE: we apply this as an optimization to avoid too much cloning
fn unfurl_pipeline(mut pipeline: Pipeline) -> Pipeline {
    loop {
        match pipeline.pop() {
            None => break,
            Some(arg) => {
                if let Expr::Func(mut call) = arg {
                    if call.count_underscores() != 1 {
                        pipeline.push(Expr::Func(call));
                        break;
                    }
                    match pipeline.pop() {
                        Some(previous_arg) => {
                            call.fill_underscore(&previous_arg);
                            pipeline.push(Expr::Func(call));
                        }
                        None => {
                            pipeline.push(Expr::Func(call));
                            break;
                        }
                    }
                } else {
                    pipeline.push(arg);
                    break;
                }
            }
        }
    }

    pipeline
}

fn optimize_pipeline(mut pipeline: Pipeline) -> Pipeline {
    pipeline = handle_pipeline_elision(pipeline);
    pipeline = trim_pipeline(pipeline);
    pipeline = unfurl_pipeline(pipeline);

    pipeline
}

pub fn parse_and_optimize_pipeline(input: &str) -> Result<Pipeline, ParseError> {
    parse_pipeline(input).map(optimize_pipeline)
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
    let mut pairs =
        MoonbladePestParser::parse(Rule::named_aggs, input).map_err(ParseError::from_pest_error)?;

    let first_pair = pairs.next().unwrap();

    debug_assert!(matches!(first_pair.as_rule(), Rule::named_aggs));

    first_pair
        .into_inner()
        .filter(|p| !matches!(p.as_rule(), Rule::EOI))
        .map(|p| {
            let (agg_name, expr_key, p) = match p.as_rule() {
                Rule::func => (
                    p.as_span().as_str().to_string(),
                    p.clone()
                        .into_inner()
                        .skip(1)
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
                            .map(|s| s.as_span().as_str())
                            .collect::<String>(),
                        func,
                    )
                }
                _ => unreachable!(),
            };

            let token_tree = TokenTree::from(p);

            let expr = MoonbladePrattParser
                .parse(&mut vec![token_tree].into_iter())
                .map_err(|err| ParseError::PrattError(err.to_string()))?;

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

    fn sid(name: &str) -> Expr {
        SpecialIdentifier(name.to_string())
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
        assert_eq!(parse_expression("%index"), Ok(sid("index")));
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
    fn test_pipeline() {
        assert_eq!(
            parse_pipeline("inc(count) | len(_)"),
            Ok(vec![
                func("inc", vec![id("count")]),
                func("len", vec![Underscore])
            ])
        );

        assert_eq!(
            parse_pipeline("count + 1 | len(_)"),
            Ok(vec![
                func("add", vec![id("count"), Int(1)]),
                func("len", vec![Underscore])
            ])
        );
    }

    #[test]
    fn test_pipeline_elision() {
        let pipeline = parse_pipeline("inc(count) | len").map(|p| handle_pipeline_elision(p));

        assert_eq!(
            pipeline,
            Ok(vec![
                func("inc", vec![id("count")]),
                func("len", vec![Underscore])
            ])
        );
    }

    #[test]
    fn test_trim_pipeline() {
        // Should give: add(a, b) | len
        let pipeline = parse_pipeline("trim(a) | add(a, b) | trim | add(a, b) | len").unwrap();
        let pipeline = handle_pipeline_elision(pipeline);
        let pipeline = trim_pipeline(pipeline);

        assert_eq!(
            pipeline,
            vec![
                func("add", vec![id("a"), id("b")]),
                func("len", vec![Underscore])
            ]
        );

        // Should give: 45 | inc
        let pipeline = parse_pipeline("trim(a) | 45 | inc").unwrap();
        let pipeline = handle_pipeline_elision(pipeline);
        let pipeline = trim_pipeline(pipeline);

        assert_eq!(pipeline, vec![Int(45), func("inc", vec![Underscore])]);

        let pipeline = parse_pipeline("trim(a) | len | add(b, _)").unwrap();
        let pipeline = handle_pipeline_elision(pipeline);
        let pipeline = trim_pipeline(pipeline);

        assert_eq!(
            pipeline,
            vec![
                func("trim", vec![id("a")]),
                func("len", vec![Underscore]),
                func("add", vec![id("b"), Underscore]),
            ]
        );
    }

    #[test]
    fn test_unfurl_pipeline() {
        // Should give: add(b, len(trim(a)))
        let pipeline = parse_pipeline("trim(a) | len | add(b, _)").unwrap();
        let pipeline = handle_pipeline_elision(pipeline);
        let pipeline = unfurl_pipeline(pipeline);

        assert_eq!(
            pipeline,
            vec![func(
                "add",
                vec![id("b"), func("len", vec![func("trim", vec![id("a")])])]
            )]
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
}
