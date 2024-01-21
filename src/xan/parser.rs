// En tant que chef, je m'engage à ce que nous ne nous fassions pas *tous* tuer.
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1, anychar, char, digit1, none_of, space0},
    combinator::{all_consuming, consumed, map, map_res, not, opt, recognize, value},
    multi::{fold_many0, many0, separated_list0, separated_list1},
    number::complete::double,
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};

use super::utils::downgrade_float;

#[derive(Debug, PartialEq, Clone)]
pub enum Argument {
    Identifier(String),
    SpecialIdentifier(String),
    StringLiteral(String),
    FloatLiteral(f64),
    IntegerLiteral(i64),
    BooleanLiteral(bool),
    RegexLiteral(String),
    Call(FunctionCall),
    Underscore,
    Null,
}

impl Argument {
    pub fn has_underscore(&self) -> bool {
        match self {
            Self::Call(call) => call.has_underscore(),
            _ => false,
        }
    }

    pub fn try_to_usize(&self) -> Option<usize> {
        match self {
            Self::IntegerLiteral(n) => {
                if *n < 0 {
                    None
                } else {
                    Some(*n as usize)
                }
            }
            Self::FloatLiteral(f) => match downgrade_float(*f) {
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

#[derive(Debug, PartialEq, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<Argument>,
}

impl FunctionCall {
    pub fn has_underscore(&self) -> bool {
        self.args.iter().any(|arg| match arg {
            Argument::Call(sub_function_call) => sub_function_call.has_underscore(),
            Argument::Underscore => true,
            _ => false,
        })
    }

    pub fn count_underscores(&self) -> usize {
        self.args
            .iter()
            .map(|arg| match arg {
                Argument::Call(sub_function_call) => sub_function_call.count_underscores(),
                Argument::Underscore => 1,
                _ => 0,
            })
            .sum()
    }

    pub fn fill_underscore(&mut self, with: &Argument) {
        match with {
            Argument::Call(_) => {
                for arg in self.args.iter_mut() {
                    match arg {
                        Argument::Call(sub) => {
                            sub.fill_underscore(with);
                        }
                        Argument::Underscore => {
                            *arg = with.clone();
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }
    }
}

pub type Pipeline = Vec<Argument>;
pub type Aggregations = Vec<Aggregation>;

#[derive(Debug, PartialEq)]
pub struct Aggregation {
    pub name: String,
    pub args: Vec<Argument>,
    pub method: String,
    pub key: String,
}

fn boolean_literal(input: &str) -> IResult<&str, bool> {
    alt((value(true, tag("true")), value(false, tag("false"))))(input)
}

fn underscore_literal(input: &str) -> IResult<&str, ()> {
    value((), char('_'))(input)
}

fn null_literal(input: &str) -> IResult<&str, ()> {
    value((), tag("null"))(input)
}

fn integer_literal<T>(input: &str) -> IResult<&str, T>
where
    T: std::str::FromStr,
{
    map_res(
        recognize(pair(
            alt((digit1, tag("-"))),
            many0(alt((digit1, tag("_")))),
        )),
        |string: &str| string.replace('_', "").parse::<T>(),
    )(input)
}

fn float_literal(input: &str) -> IResult<&str, f64> {
    double(input)
}

fn unescape(c: char, delimiter: char) -> Result<char, ()> {
    if c == delimiter {
        return Ok(c);
    }

    Ok(match c {
        '\\' | '/' => c,
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        _ => return Err(()),
    })
}

fn double_quote_string_character_literal(input: &str) -> IResult<&str, char> {
    let (input, c) = none_of("\"")(input)?;

    if c == '\\' {
        let (input, c) = anychar(input)?;

        match unescape(c, '"') {
            Ok(c) => Ok((input, c)),
            Err(_) => Err(nom::Err::Failure(nom::error::ParseError::from_char(
                input, c,
            ))),
        }
    } else {
        Ok((input, c))
    }
}

fn single_quote_string_character_literal(input: &str) -> IResult<&str, char> {
    let (input, c) = none_of("'")(input)?;

    if c == '\\' {
        let (input, c) = anychar(input)?;

        match unescape(c, '\'') {
            Ok(c) => Ok((input, c)),
            Err(_) => Err(nom::Err::Failure(nom::error::ParseError::from_char(
                input, c,
            ))),
        }
    } else {
        Ok((input, c))
    }
}

fn regex_character_literal(input: &str) -> IResult<&str, char> {
    let (input, c) = none_of("/")(input)?;

    if c == '\\' {
        let (input2, c2) = anychar(input)?;

        if c2 == '/' {
            Ok((input2, c2))
        } else {
            Ok((input, c))
        }
    } else {
        Ok((input, c))
    }
}

fn string_literal(input: &str) -> IResult<&str, String> {
    alt((
        delimited(
            char('"'),
            fold_many0(
                double_quote_string_character_literal,
                String::new,
                |mut string, c| {
                    string.push(c);
                    string
                },
            ),
            char('"'),
        ),
        delimited(
            char('\''),
            fold_many0(
                single_quote_string_character_literal,
                String::new,
                |mut string, c| {
                    string.push(c);
                    string
                },
            ),
            char('\''),
        ),
    ))(input)
}

fn regex_literal(input: &str) -> IResult<&str, String> {
    map(
        pair(
            delimited(
                char('/'),
                fold_many0(regex_character_literal, String::new, |mut string, c| {
                    string.push(c);
                    string
                }),
                char('/'),
            ),
            opt(tag("i")),
        ),
        |(pattern, i)| match i {
            None => pattern,
            Some(_) => {
                let mut case_insensitive_pattern = String::from("(?i)");
                case_insensitive_pattern.push_str(&pattern);
                case_insensitive_pattern
            }
        },
    )(input)
}

fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alpha1,
        many0(alt((alphanumeric1, tag("_"), tag("-"), tag(" ")))),
    ))(input)
}

fn special_identifier(input: &str) -> IResult<&str, &str> {
    preceded(
        char('%'),
        recognize(pair(
            alpha1,
            many0(alt((alphanumeric1, tag("_"), tag("-"), tag(" ")))),
        )),
    )(input)
}

fn restricted_identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alpha1,
        many0(alt((alphanumeric1, tag("_"), tag("-")))),
    ))(input)
}

fn comma_separator(input: &str) -> IResult<&str, ()> {
    value((), tuple((space0, char(','), space0)))(input)
}

fn argument_with_parsed(input: &str) -> IResult<&str, (&str, Argument)> {
    consumed(alt((
        function_call,
        map(boolean_literal, Argument::BooleanLiteral),
        map(null_literal, |_| Argument::Null),
        map(special_identifier, |name| {
            Argument::SpecialIdentifier(String::from(name))
        }),
        map(identifier, |name| Argument::Identifier(String::from(name))),
        map(terminated(integer_literal, not(char('.'))), |value| {
            Argument::IntegerLiteral(value)
        }),
        map(float_literal, Argument::FloatLiteral),
        map(regex_literal, Argument::RegexLiteral),
        map(string_literal, Argument::StringLiteral),
        map(underscore_literal, |_| Argument::Underscore),
    )))(input)
}

fn argument(input: &str) -> IResult<&str, Argument> {
    map(argument_with_parsed, |arg| arg.1)(input)
}

fn argument_list_with_parsed(input: &str) -> IResult<&str, Vec<(&str, Argument)>> {
    separated_list0(comma_separator, argument_with_parsed)(input)
}

fn argument_list(input: &str) -> IResult<&str, Vec<Argument>> {
    separated_list0(comma_separator, argument)(input)
}

fn function_call(input: &str) -> IResult<&str, Argument> {
    map(
        pair(
            restricted_identifier,
            delimited(
                pair(space0, char('(')),
                argument_list,
                pair(char(')'), space0),
            ),
        ),
        |(name, args)| {
            Argument::Call(FunctionCall {
                name: name.to_lowercase(),
                args,
            })
        },
    )(input)
}

fn possibly_elided_function_call(input: &str) -> IResult<&str, Argument> {
    map(
        pair(
            restricted_identifier,
            opt(delimited(
                pair(space0, char('(')),
                argument_list,
                pair(char(')'), space0),
            )),
        ),
        |(name, args)| {
            Argument::Call(FunctionCall {
                name: name.to_lowercase(),
                args: args.unwrap_or_else(|| vec![Argument::Underscore]),
            })
        },
    )(input)
}

fn pipe(input: &str) -> IResult<&str, ()> {
    value((), tuple((space0, char('|'), space0)))(input)
}

fn pipeline(input: &str) -> IResult<&str, Pipeline> {
    all_consuming(separated_list1(pipe, possibly_elided_function_call))(input)
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
                if let Argument::Call(mut call) = arg {
                    if call.count_underscores() != 1 {
                        pipeline.push(Argument::Call(call));
                        break;
                    }
                    match pipeline.pop() {
                        Some(previous_arg) => {
                            call.fill_underscore(&previous_arg);
                            pipeline.push(Argument::Call(call));
                        }
                        None => {
                            pipeline.push(Argument::Call(call));
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

fn as_suffix(input: &str) -> IResult<&str, String> {
    preceded(
        tuple((space0, tag("as"), space0)),
        alt((
            string_literal,
            map(restricted_identifier, |id| id.to_string()),
        )),
    )(input)
}

fn aggregation(input: &str) -> IResult<&str, Aggregation> {
    map(
        tuple((
            restricted_identifier,
            consumed(delimited(
                pair(space0, char('(')),
                argument_list_with_parsed,
                pair(char(')'), space0),
            )),
            opt(as_suffix),
        )),
        |(method, (expr, args_with_expr), name)| {
            let key = match args_with_expr.get(0) {
                None => "".to_string(),
                Some((expr, _)) => expr.trim().to_string(),
            };

            let args: Vec<Argument> = args_with_expr.into_iter().map(|t| t.1).collect();

            Aggregation {
                name: name.map(|n| n.to_string()).unwrap_or_else(|| {
                    let mut prefix = String::from(method);
                    prefix.push_str(expr.trim());
                    prefix
                }),
                args,
                method: String::from(method),
                key,
            }
        },
    )(input)
}

fn aggregations(input: &str) -> IResult<&str, Aggregations> {
    all_consuming(separated_list1(comma_separator, aggregation))(input)
}

// fn optimize_aggregations(mut aggregations: Aggregations) -> Aggregations {
//     let (empty_aggs, non_empty_aggs): (Vec<_>, Vec<_>) =
//         aggregations.iter_mut().partition(|agg| agg.args.is_empty());

//     if empty_aggs.is_empty() {
//         return aggregations;
//     }

//     if let Some(any_non_empty_agg) = non_empty_aggs.get(0) {
//         for empty_agg in empty_aggs {
//             empty_agg.key = any_non_empty_agg.key.clone();
//         }
//     }

//     aggregations
// }

// NOTE: the parse functions return a now useless Result (compared to an Option)
// because they might return something more useful in the future.
pub fn parse_pipeline(code: &str) -> Result<Pipeline, ()> {
    match pipeline(code) {
        Ok(p) => Ok(p.1),
        Err(_) => Err(()),
    }
}

pub fn parse_and_optimize_pipeline(code: &str) -> Result<Pipeline, ()> {
    parse_pipeline(code).map(|pipeline| unfurl_pipeline(trim_pipeline(pipeline)))
}

pub fn parse_aggregations(code: &str) -> Result<Aggregations, ()> {
    match aggregations(code) {
        Ok(p) => Ok(p.1),
        Err(_) => Err(()),
    }
}

// pub fn parse_and_optimize_aggregations(code: &str) -> Result<Aggregations, ()> {
//     parse_aggregations(code).map(|aggregations| optimize_aggregations(aggregations))
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boolean_literal() {
        assert_eq!(boolean_literal("true, test"), Ok((", test", true)));

        assert_eq!(boolean_literal("false"), Ok(("", false)));
    }

    #[test]
    fn test_float_literal() {
        assert_eq!(float_literal("3.56"), Ok(("", 3.56f64)))
    }

    #[test]
    fn test_integer_literal() {
        assert_eq!(integer_literal("456_400"), Ok(("", 456_400i64)));
        assert_eq!(integer_literal("-36, test"), Ok((", test", -36i64)));
    }

    #[test]
    fn test_string_literal() {
        assert_eq!(string_literal("\"\", 45"), Ok((", 45", String::from(""))));
        assert_eq!(string_literal("'', 45"), Ok((", 45", String::from(""))));
        assert_eq!(
            string_literal(r#""hello", 45"#),
            Ok((", 45", String::from("hello")))
        );
        assert_eq!(
            string_literal(r#""héllo", 45"#),
            Ok((", 45", String::from("héllo")))
        );
        assert_eq!(
            string_literal(r#""hel\nlo", 45"#),
            Ok((", 45", String::from("hel\nlo")))
        );
        assert_eq!(
            string_literal(r#""hello \"world\"", 45"#),
            Ok((", 45", String::from("hello \"world\"")))
        );
        assert_eq!(
            string_literal(r#"'hello \'world\'', 45"#),
            Ok((", 45", String::from("hello 'world'")))
        );
    }

    #[test]
    fn test_regex_literal() {
        assert_eq!(
            regex_literal(r#"/test/, ok"#),
            Ok((", ok", "test".to_string()))
        );

        assert_eq!(
            regex_literal(r#"/\nok[a]./, ok"#),
            Ok((", ok", "\\nok[a].".to_string()))
        );

        assert_eq!(
            regex_literal(r#"/\r/, ok"#),
            Ok((", ok", "\\r".to_string()))
        );

        assert_eq!(regex_literal(r#"/\//, ok"#), Ok((", ok", "/".to_string())));

        assert_eq!(regex_literal("/test/i"), Ok(("", "(?i)test".to_string())));
    }

    #[test]
    fn test_underscore_literal() {
        assert_eq!(underscore_literal("_, 45"), Ok((", 45", ())))
    }

    #[test]
    fn test_identifier() {
        assert_eq!(
            restricted_identifier("input, test"),
            Ok((", test", "input"))
        );
        assert_eq!(
            identifier("PREFIXES AS URL, test"),
            Ok((", test", "PREFIXES AS URL"))
        );
        assert_eq!(special_identifier("%index, ok"), Ok((", ok", "index")));
    }

    #[test]
    fn test_argument() {
        assert_eq!(argument("true"), Ok(("", Argument::BooleanLiteral(true))));
        assert_eq!(
            argument("\"test\""),
            Ok(("", Argument::StringLiteral(String::from("test"))))
        );
        assert_eq!(
            argument("/test/, name"),
            Ok((", name", Argument::RegexLiteral(String::from("test"))))
        );
    }

    #[test]
    fn test_argument_list() {
        assert_eq!(argument_list(""), Ok(("", vec![])));
        assert_eq!(
            argument_list("true, _, col0"),
            Ok((
                "",
                vec![
                    Argument::BooleanLiteral(true),
                    Argument::Underscore,
                    Argument::Identifier(String::from("col0"))
                ]
            ))
        )
    }

    #[test]
    fn test_function_call() {
        assert_eq!(
            possibly_elided_function_call("trim()"),
            Ok((
                "",
                Argument::Call(FunctionCall {
                    name: String::from("trim"),
                    args: vec![]
                })
            ))
        );

        assert_eq!(
            possibly_elided_function_call("trim(_)"),
            Ok((
                "",
                Argument::Call(FunctionCall {
                    name: String::from("trim"),
                    args: vec![Argument::Underscore]
                })
            ))
        );

        assert_eq!(
            possibly_elided_function_call("trim(_, true, 4.5, 56, col)"),
            Ok((
                "",
                Argument::Call(FunctionCall {
                    name: String::from("trim"),
                    args: vec![
                        Argument::Underscore,
                        Argument::BooleanLiteral(true),
                        Argument::FloatLiteral(4.5),
                        Argument::IntegerLiteral(56),
                        Argument::Identifier(String::from("col"))
                    ]
                })
            ))
        );
    }

    #[test]
    fn test_pipeline() {
        assert!(pipeline("test |").is_err());

        assert_eq!(
            pipeline("trim(name) | len  (_)"),
            Ok((
                "",
                vec![
                    Argument::Call(FunctionCall {
                        name: String::from("trim"),
                        args: vec![Argument::Identifier(String::from("name"))]
                    }),
                    Argument::Call(FunctionCall {
                        name: String::from("len"),
                        args: vec![Argument::Underscore]
                    })
                ]
            ))
        );

        assert_eq!(
            pipeline("add(len(name), len(surname)) | len  (_)"),
            Ok((
                "",
                vec![
                    Argument::Call(FunctionCall {
                        name: String::from("add"),
                        args: vec![
                            Argument::Call(FunctionCall {
                                name: "len".to_string(),
                                args: vec![Argument::Identifier("name".to_string())]
                            }),
                            Argument::Call(FunctionCall {
                                name: "len".to_string(),
                                args: vec![Argument::Identifier("surname".to_string())]
                            })
                        ]
                    }),
                    Argument::Call(FunctionCall {
                        name: String::from("len"),
                        args: vec![Argument::Underscore]
                    })
                ]
            ))
        );

        assert_eq!(
            pipeline("if(true, len(name), len(surname))"),
            Ok((
                "",
                vec![Argument::Call(FunctionCall {
                    name: String::from("if"),
                    args: vec![
                        Argument::BooleanLiteral(true),
                        Argument::Call(FunctionCall {
                            name: "len".to_string(),
                            args: vec![Argument::Identifier("name".to_string())]
                        }),
                        Argument::Call(FunctionCall {
                            name: "len".to_string(),
                            args: vec![Argument::Identifier("surname".to_string())]
                        })
                    ]
                })]
            ))
        );

        assert_eq!(
            pipeline("trim(name)|len  (_)"),
            Ok((
                "",
                vec![
                    Argument::Call(FunctionCall {
                        name: String::from("trim"),
                        args: vec![Argument::Identifier(String::from("name"))]
                    }),
                    Argument::Call(FunctionCall {
                        name: String::from("len"),
                        args: vec![Argument::Underscore]
                    })
                ]
            ))
        );

        assert_eq!(
            pipeline("trim(name) | len(_)  "),
            Ok((
                "",
                vec![
                    Argument::Call(FunctionCall {
                        name: String::from("trim"),
                        args: vec![Argument::Identifier(String::from("name"))]
                    }),
                    Argument::Call(FunctionCall {
                        name: String::from("len"),
                        args: vec![Argument::Underscore]
                    })
                ]
            ))
        );

        assert_eq!(
            pipeline("trim | len | coalesce(null)"),
            Ok((
                "",
                vec![
                    Argument::Call(FunctionCall {
                        name: String::from("trim"),
                        args: vec![Argument::Underscore]
                    }),
                    Argument::Call(FunctionCall {
                        name: String::from("len"),
                        args: vec![Argument::Underscore]
                    }),
                    Argument::Call(FunctionCall {
                        name: String::from("coalesce"),
                        args: vec![Argument::Null]
                    })
                ]
            ))
        );
    }

    #[test]
    fn test_trim_pipeline() {
        // Should give: add(a, b) | len
        let pipeline = parse_pipeline("trim(a) | add(a, b) | trim | add(a, b) | len").unwrap();
        let pipeline = trim_pipeline(pipeline);

        assert_eq!(
            pipeline,
            vec![
                Argument::Call(FunctionCall {
                    name: "add".to_string(),
                    args: vec![
                        Argument::Identifier("a".to_string()),
                        Argument::Identifier("b".to_string())
                    ]
                }),
                Argument::Call(FunctionCall {
                    name: "len".to_string(),
                    args: vec![Argument::Underscore]
                })
            ]
        );

        let pipeline = parse_pipeline("trim(a) | len | add(b, _)").unwrap();
        let pipeline = trim_pipeline(pipeline);

        assert_eq!(
            pipeline,
            vec![
                Argument::Call(FunctionCall {
                    name: "trim".to_string(),
                    args: vec![Argument::Identifier("a".to_string())]
                }),
                Argument::Call(FunctionCall {
                    name: "len".to_string(),
                    args: vec![Argument::Underscore]
                }),
                Argument::Call(FunctionCall {
                    name: "add".to_string(),
                    args: vec![Argument::Identifier("b".to_string()), Argument::Underscore]
                })
            ]
        );
    }

    #[test]
    fn test_unfurl_pipeline() {
        // Should give: add(b, len(trim(a)))
        let pipeline = parse_pipeline("trim(a) | len | add(b, _)").unwrap();
        let pipeline = unfurl_pipeline(pipeline);

        assert_eq!(
            pipeline,
            vec![Argument::Call(FunctionCall {
                name: "add".to_string(),
                args: vec![
                    Argument::Identifier("b".to_string()),
                    Argument::Call(FunctionCall {
                        name: "len".to_string(),
                        args: vec![Argument::Call(FunctionCall {
                            name: "trim".to_string(),
                            args: vec![Argument::Identifier("a".to_string())]
                        })]
                    })
                ]
            })]
        );
    }

    #[test]
    fn test_as_suffix() {
        assert_eq!(
            as_suffix("as name, test"),
            Ok((", test", "name".to_string()))
        );
        assert_eq!(
            as_suffix("  as    name, test"),
            Ok((", test", "name".to_string()))
        );
        assert_eq!(
            as_suffix("as \"name2\", test"),
            Ok((", test", "name2".to_string()))
        );
    }

    #[test]
    fn test_aggregation() {
        assert_eq!(
            aggregation("mean(A)"),
            Ok((
                "",
                Aggregation {
                    name: "mean(A)".to_string(),
                    method: "mean".to_string(),
                    args: vec![Argument::Identifier("A".to_string())],
                    key: "A".to_string()
                }
            ))
        );
        assert_eq!(
            aggregation("mean(A) as avg"),
            Ok((
                "",
                Aggregation {
                    name: "avg".to_string(),
                    method: "mean".to_string(),
                    args: vec![Argument::Identifier("A".to_string())],
                    key: "A".to_string()
                }
            ))
        );
        assert_eq!(
            aggregation("mean(add(A, B))"),
            Ok((
                "",
                Aggregation {
                    name: "mean(add(A, B))".to_string(),
                    method: "mean".to_string(),
                    args: vec![Argument::Call(FunctionCall {
                        name: "add".to_string(),
                        args: vec![
                            Argument::Identifier("A".to_string()),
                            Argument::Identifier("B".to_string())
                        ]
                    })],
                    key: "add(A, B)".to_string()
                }
            ))
        );
    }

    #[test]
    fn test_aggregations() {
        assert_eq!(
            aggregations("mean(add(A, B)), sum(C) as s"),
            Ok((
                "",
                vec![
                    Aggregation {
                        name: "mean(add(A, B))".to_string(),
                        method: "mean".to_string(),
                        args: vec![Argument::Call(FunctionCall {
                            name: "add".to_string(),
                            args: vec![
                                Argument::Identifier("A".to_string()),
                                Argument::Identifier("B".to_string())
                            ]
                        })],
                        key: "add(A, B)".to_string()
                    },
                    Aggregation {
                        name: "s".to_string(),
                        method: "sum".to_string(),
                        args: vec![Argument::Identifier("C".to_string())],
                        key: "C".to_string()
                    }
                ]
            ))
        );
    }

    // #[test]
    // fn test_optimize_aggregations() {
    //     let aggregations = parse_aggregations("mean(A), sum(A)").unwrap();
    //     let aggregations = optimize_aggregations(aggregations);

    //     assert_eq!(
    //         aggregations,
    //         vec![
    //             Aggregation {
    //                 name: "mean(A)".to_string(),
    //                 method: "mean".to_string(),
    //                 args: vec![Argument::Identifier("A".to_string())],
    //                 key: "A".to_string()
    //             },
    //             Aggregation {
    //                 name: "sum(A)".to_string(),
    //                 method: "sum".to_string(),
    //                 args: vec![Argument::Identifier("A".to_string())],
    //                 key: "A".to_string()
    //             },
    //         ]
    //     );

    //     let aggregations = parse_aggregations("count(), sum(A)").unwrap();
    //     let aggregations = optimize_aggregations(aggregations);

    //     assert_eq!(
    //         aggregations,
    //         vec![
    //             Aggregation {
    //                 name: "count()".to_string(),
    //                 method: "count".to_string(),
    //                 args: vec![],
    //                 key: "A".to_string()
    //             },
    //             Aggregation {
    //                 name: "sum(A)".to_string(),
    //                 method: "sum".to_string(),
    //                 args: vec![Argument::Identifier("A".to_string())],
    //                 key: "A".to_string()
    //             },
    //         ]
    //     );
    // }
}
