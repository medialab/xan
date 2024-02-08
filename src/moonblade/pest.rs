use pest::iterators::Pair;
use pest_derive::Parser;
use pratt::{Affix, Associativity, PrattParser, Precedence};

#[derive(Parser)]
#[grammar = "moonblade/grammar.pest"]
pub struct MoonbladePestParser;

#[derive(Debug)]
enum Operator {
    Add,
}

#[derive(Debug)]
enum TokenTree<'a> {
    Infix(Operator),
    Primary(Pair<'a, Rule>),
    Expr(Vec<TokenTree<'a>>),
    Func(String, Vec<TokenTree<'a>>),
}

impl<'a> From<Pair<'a, Rule>> for TokenTree<'a> {
    fn from(pair: Pair<'a, Rule>) -> Self {
        match pair.as_rule() {
            Rule::int | Rule::ident => TokenTree::Primary(pair),
            Rule::add => TokenTree::Infix(Operator::Add),
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

#[derive(Debug)]
pub enum Expr {
    Func(String, Vec<Expr>),
    Int(i64),
    Identifier(String),
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
            TokenTree::Infix(Operator::Add) => Affix::Infix(Precedence(2), Associativity::Left),
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
                        .parse::<i64>()
                        .or(Err("could not parse int"))?;

                    Expr::Int(n)
                }
                Rule::ident => Expr::Identifier(token.to_string()),
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
        Ok(match tree {
            TokenTree::Infix(op) => match op {
                Operator::Add => Expr::Func("add".to_string(), vec![lhs, rhs]),
            },
            _ => unreachable!(),
        })
    }

    fn prefix(&mut self, _tree: TokenTree, _rhs: Expr) -> Result<Expr, Self::Error> {
        unreachable!()
    }

    fn postfix(&mut self, _lhs: Expr, _tree: TokenTree) -> Result<Expr, Self::Error> {
        unreachable!()
    }
}

mod tests {
    use super::*;
    use pest::Parser;

    #[test]
    fn test_basics() {
        let token_tree = TokenTree::from(
            MoonbladePestParser::parse(Rule::full_expr, "1 + add(1, name + 3)")
                .unwrap()
                .next()
                .unwrap(),
        );

        dbg!(&token_tree);

        let expr = MoonbladePrattParser.parse(&mut vec![token_tree].into_iter());

        dbg!(expr);
    }
}
