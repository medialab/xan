use std::convert::TryFrom;

use pest::iterators::Pair;
use pest_derive::Parser;
use pratt::{Affix, Associativity, PrattParser, Precedence, Result as PResult};

#[derive(Parser)]
#[grammar = "moonblade/grammar.pest"]
pub struct MoonbladePestParser;

#[derive(Debug)]
enum Token {
    Identifier(String),
    Int(i64),
}

#[derive(Debug)]
enum Operator {
    Add,
}

#[derive(Debug)]
enum TokenTree {
    Infix(Operator),
    Primary(Token),
    Expr(Vec<TokenTree>),
    Func(String, Vec<TokenTree>),
}

impl<'a> TryFrom<Pair<'a, Rule>> for TokenTree {
    type Error = ();

    fn try_from(pair: Pair<Rule>) -> Result<Self, Self::Error> {
        Ok(match pair.as_rule() {
            Rule::int => {
                let n = pair.as_str().parse::<i64>();

                TokenTree::Primary(Token::Int(n.expect(&format!("{:?}", pair))))
            }
            Rule::add => TokenTree::Infix(Operator::Add),
            Rule::expr => {
                let mut pairs = pair.into_inner();

                if pairs.len() == 1 {
                    Self::try_from(pairs.next().unwrap())?
                } else {
                    TokenTree::Expr(
                        pairs
                            .map(|p| Self::try_from(p))
                            .collect::<Result<Vec<_>, ()>>()?,
                    )
                }
            }
            Rule::func => {
                let mut pairs = pair.into_inner();
                let func_name = pairs.next().unwrap().as_str().to_string();

                TokenTree::Func(
                    func_name,
                    pairs
                        .map(|p| Self::try_from(p))
                        .collect::<Result<Vec<_>, ()>>()?,
                )
            }
            _ => {
                dbg!(&pair);
                unreachable!();
            }
        })
    }
}

#[derive(Debug)]
pub enum Expr {
    Func(String, Vec<Expr>),
    Int(i64),
    Identifier(String),
}

struct MoonbladePrattParser;

impl<I> PrattParser<I> for MoonbladePrattParser
where
    I: Iterator<Item = TokenTree>,
{
    type Error = pratt::NoError;
    type Input = TokenTree;
    type Output = Expr;

    fn query(&mut self, tree: &TokenTree) -> PResult<Affix> {
        let affix = match tree {
            TokenTree::Infix(Operator::Add) => Affix::Infix(Precedence(2), Associativity::Left),
            TokenTree::Expr(_) => Affix::Nilfix,
            TokenTree::Func(_, _) => Affix::Nilfix,
            TokenTree::Primary(_) => Affix::Nilfix,
        };

        Ok(affix)
    }

    fn primary(&mut self, tree: TokenTree) -> PResult<Expr> {
        let expr = match tree {
            TokenTree::Primary(token) => match token {
                Token::Identifier(name) => Expr::Identifier(name),
                Token::Int(n) => Expr::Int(n),
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

    fn infix(&mut self, lhs: Expr, tree: TokenTree, rhs: Expr) -> PResult<Expr> {
        Ok(match tree {
            TokenTree::Infix(op) => match op {
                Operator::Add => Expr::Func("add".to_string(), vec![lhs, rhs]),
            },
            _ => unreachable!(),
        })
    }

    fn prefix(&mut self, _tree: TokenTree, _rhs: Expr) -> PResult<Expr> {
        unreachable!()
    }

    fn postfix(&mut self, _lhs: Expr, _tree: TokenTree) -> PResult<Expr> {
        unreachable!()
    }
}

mod tests {
    use super::*;
    use pest::Parser;

    #[test]
    fn test_basics() {
        let token_tree = TokenTree::try_from(
            MoonbladePestParser::parse(Rule::full_expr, "1 + add(1, 2 + 3)")
                .unwrap()
                .next()
                .unwrap(),
        )
        .unwrap();

        dbg!(&token_tree);

        let expr = MoonbladePrattParser.parse(&mut vec![token_tree].into_iter());

        dbg!(expr);
    }
}
