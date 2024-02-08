use pest::iterators::Pair;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "moonblade/grammar.pest"]
pub struct MoonbladeParser;

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
    Func((String, Vec<TokenTree>)),
}

fn pair_to_token_tree(pair: Pair<Rule>) -> Result<TokenTree, ()> {
    Ok(match pair.as_rule() {
        Rule::int => {
            let n = pair.as_str().parse::<i64>();

            TokenTree::Primary(Token::Int(n.expect(&format!("{:?}", pair))))
        }
        Rule::add => TokenTree::Infix(Operator::Add),
        Rule::expr => {
            let mut pairs = pair.into_inner();

            if pairs.len() == 1 {
                pair_to_token_tree(pairs.next().unwrap())?
            } else {
                TokenTree::Expr(
                    pairs
                        .map(|p| pair_to_token_tree(p))
                        .collect::<Result<Vec<_>, ()>>()?,
                )
            }
        }
        Rule::func => {
            let mut pairs = pair.into_inner();
            let func_name = pairs.next().unwrap().as_str().to_string();

            TokenTree::Func((
                func_name,
                pairs
                    .map(|p| pair_to_token_tree(p))
                    .collect::<Result<Vec<_>, ()>>()?,
            ))
        }
        _ => {
            dbg!(&pair);
            unreachable!();
        }
    })
}

mod tests {
    use super::*;
    use pest::Parser;

    #[test]
    fn test_basics() {
        dbg!(pair_to_token_tree(
            MoonbladeParser::parse(Rule::full_expr, "1 + add(1, 2 + 3)")
                .unwrap()
                .next()
                .unwrap(),
        ));
    }
}
