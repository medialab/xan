use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "moonblade/grammar.pest"]
pub struct MoonbladeParser;

mod tests {
    use super::*;
    use pest::Parser;

    #[test]
    fn test_basics() {
        dbg!(MoonbladeParser::parse(Rule::expr, "add(1, 2)"));
    }
}
