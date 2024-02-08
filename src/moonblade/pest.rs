use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use pratt::{Affix, Associativity, PrattParser, Precedence};

use super::functions::get_function;

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
            Rule::int | Rule::float | Rule::ident | Rule::underscore => TokenTree::Primary(pair),
            Rule::add => TokenTree::Infix(Operator::Add),
            Rule::mul => TokenTree::Infix(Operator::Mul),
            Rule::not => TokenTree::Infix(Operator::Not),
            Rule::expr => {
                let mut pairs = pair.into_inner();

                if pairs.len() == 1 {
                    Self::from(pairs.next().unwrap())
                } else {
                    TokenTree::Expr(pairs.map(|p| Self::from(p)).collect())
                }
            }
            Rule::func => {
                let mut pairs = pair.into_inner();
                let func_name = pairs.next().unwrap().as_str().to_string();

                TokenTree::Func(func_name, pairs.map(|p| Self::from(p)).collect())
            }
            _ => {
                dbg!(&pair);
                unreachable!();
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Expr {
    Func(String, Vec<Expr>),
    Int(i64),
    Float(f64),
    Identifier(String),
    Underscore,
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
                        .replace("_", "")
                        .parse::<i64>()
                        .or(Err("could not parse int"))?;

                    Expr::Int(n)
                }
                Rule::float => {
                    let n = token
                        .as_str()
                        .replace("_", "")
                        .parse::<f64>()
                        .or(Err("could not parse float"))?;

                    Expr::Float(n)
                }
                Rule::underscore => Expr::Underscore,
                Rule::ident => Expr::Identifier(token.as_str().to_string()),
                _ => unreachable!(),
            },
            TokenTree::Expr(group) => self.parse(&mut group.into_iter()).unwrap(),
            TokenTree::Func(name, group) => Expr::Func(
                name,
                group
                    .into_iter()
                    .map(|g| self.parse(&mut vec![g].into_iter()).unwrap())
                    .collect(),
            ),
            _ => unreachable!(),
        };
        Ok(expr)
    }

    fn infix(&mut self, lhs: Expr, tree: TokenTree, rhs: Expr) -> Result<Expr, Self::Error> {
        let args = vec![lhs, rhs];

        Ok(match tree {
            TokenTree::Infix(op) => Expr::Func(op.to_fn_string(), args),
            _ => unreachable!(),
        })
    }

    fn prefix(&mut self, tree: TokenTree, rhs: Expr) -> Result<Expr, Self::Error> {
        let args = vec![rhs];

        Ok(match tree {
            TokenTree::Infix(op) => Expr::Func(op.to_fn_string(), args),
            _ => unreachable!(),
        })
    }

    fn postfix(&mut self, _lhs: Expr, _tree: TokenTree) -> Result<Expr, Self::Error> {
        unreachable!()
    }
}

#[derive(PartialEq, Debug)]
enum ParseError {
    PestError(pest::error::Error<Rule>),
    PrattError(String),
}

fn parse_expression(input: &str) -> Result<Expr, ParseError> {
    let mut pairs = MoonbladePestParser::parse(Rule::full_expr, input)
        .map_err(|err| ParseError::PestError(err))?;

    let first_pair = pairs.next().unwrap();

    let token_tree = TokenTree::from(first_pair);

    MoonbladePrattParser
        .parse(&mut vec![token_tree].into_iter())
        .map_err(|err| ParseError::PrattError(err.to_string()))
}

pub type Pipeline = Vec<Expr>;

// TODO: trim, unfurl, resolve identifiers as functions

fn parse_pipeline(input: &str) -> Result<Pipeline, ParseError> {
    let mut pairs = MoonbladePestParser::parse(Rule::pipeline, input)
        .map_err(|err| ParseError::PestError(err))?;

    let first_pair = pairs.next().unwrap();

    first_pair
        .into_inner()
        .into_iter()
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
                match get_function(&name) {
                    None => expr,
                    Some(_) => Expr::Func(name.to_string(), vec![Expr::Underscore]),
                }
            } else {
                expr
            }
        })
        .collect()
}

fn optimize_pipeline(pipeline: Pipeline) -> Pipeline {
    handle_pipeline_elision(pipeline)
}

#[cfg(test)]
mod tests {
    use super::Expr::*;
    use super::*;

    fn id(name: &str) -> Expr {
        Identifier(name.to_string())
    }

    fn func(name: &str, args: Vec<Expr>) -> Expr {
        Func(name.to_string(), args)
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
                func("len", vec![Expr::Underscore])
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
                func("len", vec![Expr::Underscore])
            ])
        );
    }
}
