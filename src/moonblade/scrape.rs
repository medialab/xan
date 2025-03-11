use std::sync::Arc;

use csv::ByteRecord;
use ego_tree::NodeId;
use scraper::{ElementRef, Html, Selector};

use super::error::ConcretizationError;
use super::interpreter::{concretize_expression, ConcreteExpr, EvaluationContext, GlobalVariables};
use super::parser::{parse_scraper, Expr, ScrapingBrackets, ScrapingNode};
use super::types::{Arity, DynamicValue, LambdaArguments};

trait GetElement {
    fn get_element(&self, id: NodeId) -> ElementRef;
}

impl GetElement for Html {
    fn get_element(&self, id: NodeId) -> ElementRef {
        ElementRef::wrap(self.tree.get(id).unwrap()).unwrap()
    }
}

#[derive(Debug, Clone)]
enum Selection {
    None,
    Singular(NodeId),
    Plural(Arc<Vec<NodeId>>),
}

#[derive(Debug)]
enum SelectionRoutine {
    Stay,
    One(Selector),
    All(Selector),
    Parent,
}

impl SelectionRoutine {
    fn run(&self, html: &Html, selection: &Selection) -> Selection {
        match (self, selection) {
            (_, Selection::None) | (Self::Stay, _) => selection.clone(),
            (Self::One(selector), Selection::Singular(id)) => {
                let element = html.get_element(*id);
                element
                    .select(selector)
                    .next()
                    .map(|e| Selection::Singular(e.id()))
                    .unwrap_or_else(|| Selection::None)
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
enum Extractor {
    Text,
    Attr(String),
}

impl Extractor {
    fn run(&self, html: &Html, selection: &Selection) -> Option<String> {
        match selection {
            Selection::None => None,
            Selection::Singular(id) => {
                let element = html.get_element(*id);
                Some(element.text().collect::<String>())
            }
            _ => unimplemented!(),
        }
    }
}

// NOTE: this is an enum because it will be easier to extend later on if we
// need to complexify evaluation of the selection part.
#[derive(Debug)]
enum ConcreteSelectionExpr {
    Call(SelectionRoutine, Vec<ConcreteSelectionExpr>),
}

impl ConcreteSelectionExpr {
    fn evaluate(&self, html: &Html, selection: &Selection) -> Selection {
        match self {
            Self::Call(routine, args) => {
                if let Some(first_arg) = args.first() {
                    routine.run(html, &first_arg.evaluate(html, selection))
                } else {
                    routine.run(html, selection)
                }
            }
        }
    }
}

#[derive(Debug)]
struct ConcreteScrapingLeaf {
    name: String,
    extractor: Extractor,
    processing: Option<ConcreteExpr>,
}

// impl ConcreteScrapingLeaf {
//     fn evaluate(
//         &self,
//         index: Option<usize>,
//         record: &ByteRecord,
//         context: &EvaluationContext,
//         globals: Option<&GlobalVariables>,
//         lambda_variables: Option<&LambdaArguments>,
//     ) {
//     }
// }

#[derive(Debug)]
enum ConcreteScrapingNode {
    Brackets(ConcreteScrapingBrackets),
    Leaf(ConcreteScrapingLeaf),
}

impl ConcreteScrapingNode {
    fn names(&self) -> Vec<&str> {
        let mut found = Vec::new();

        match self {
            Self::Leaf(leaf) => {
                found.push(leaf.name.as_str());
            }
            Self::Brackets(brackets) => {
                for node in brackets.nodes.iter() {
                    found.extend(node.names());
                }
            }
        };

        found
    }

    // TODO: will need the whole array of things later on
    // TODO: processing evaluation also?
    // TODO: move leaf evaluation into own struct above
    fn evaluate(&self, html: &Html, selection: &Selection) -> Vec<DynamicValue> {
        let mut found = Vec::new();

        match self {
            Self::Leaf(leaf) => {
                found.push(DynamicValue::from(leaf.extractor.run(html, selection)));
            }
            Self::Brackets(brackets) => {
                for node in brackets.nodes.iter() {
                    found.extend(node.evaluate(html, selection));
                }
            }
        };

        found
    }
}

#[derive(Debug)]
struct ConcreteScrapingBrackets {
    selection_expr: ConcreteSelectionExpr,
    nodes: Vec<ConcreteScrapingNode>,
}

impl ConcreteScrapingBrackets {
    fn names(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().flat_map(|node| node.names())
    }

    fn evaluate(&self, html: &Html, selection: &Selection) -> Vec<DynamicValue> {
        let selection = self.selection_expr.evaluate(html, selection);

        self.nodes
            .iter()
            .flat_map(|node| node.evaluate(html, &selection))
            .collect()
    }
}

#[derive(Debug)]
struct ConcreteScraper(Vec<ConcreteScrapingBrackets>);

impl ConcreteScraper {
    fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().flat_map(|brackets| brackets.names())
    }

    fn evaluate(&self, html: &Html, selection: &Selection) -> Vec<DynamicValue> {
        self.0
            .iter()
            .flat_map(|brackets| brackets.evaluate(html, selection))
            .collect()
    }
}

fn parse_selector(concrete_expr: ConcreteExpr) -> Result<Selector, ConcretizationError> {
    let value = concrete_expr.try_unwrap()?;
    let css = value
        .try_as_str()
        .map_err(|_| ConcretizationError::NotStaticallyAnalyzable)?;

    Selector::parse(&css).map_err(|_| ConcretizationError::InvalidCSSSelector(css.to_string()))
}

fn concretize_selection_expr(
    expr: Expr,
    headers: &ByteRecord,
    globals: Option<&GlobalVariables>,
) -> Result<ConcreteSelectionExpr, ConcretizationError> {
    match expr {
        Expr::Func(mut call) => {
            Arity::Range(1..=2)
                .validate(call.args.len())
                .map_err(|invalid_arity| {
                    ConcretizationError::InvalidArity(call.name.to_string(), invalid_arity)
                })?;

            let arg = call.args.pop().unwrap().1;
            let concrete_arg = concretize_expression(arg, headers, globals)?;

            let args = if call.args.is_empty() {
                vec![]
            } else {
                vec![concretize_selection_expr(
                    call.args.pop().unwrap().1,
                    headers,
                    globals,
                )?]
            };

            let selection_expr = match call.name.as_str() {
                "one" => {
                    let selector = parse_selector(concrete_arg)?;
                    ConcreteSelectionExpr::Call(SelectionRoutine::One(selector), args)
                }
                "all" => {
                    let selector = parse_selector(concrete_arg)?;
                    ConcreteSelectionExpr::Call(SelectionRoutine::All(selector), args)
                }
                _ => return Err(ConcretizationError::UnknownFunction(call.name.to_string())),
            };

            Ok(selection_expr)
        }
        Expr::Str(css) => {
            let selector = Selector::parse(&css)
                .map_err(|_| ConcretizationError::InvalidCSSSelector(css.to_string()))?;

            Ok(ConcreteSelectionExpr::Call(
                SelectionRoutine::One(selector),
                vec![],
            ))
        }
        Expr::Underscore => Ok(ConcreteSelectionExpr::Call(SelectionRoutine::Stay, vec![])),
        _ => Err(ConcretizationError::NotStaticallyAnalyzable),
    }
}

fn concretize_brackets(
    brackets: ScrapingBrackets,
    headers: &ByteRecord,
    globals: Option<&GlobalVariables>,
) -> Result<ConcreteScrapingBrackets, ConcretizationError> {
    let selection_expr = concretize_selection_expr(brackets.selection_expr, headers, globals)?;

    let nodes = brackets
        .nodes
        .into_iter()
        .map(|node| {
            Ok(match node {
                ScrapingNode::Leaf(leaf) => {
                    let concrete_leaf = ConcreteScrapingLeaf {
                        name: leaf.name,
                        extractor: Extractor::Text, // TODO
                        processing: leaf
                            .processing
                            .map(|processing| concretize_expression(processing, headers, globals))
                            .transpose()?,
                    };

                    ConcreteScrapingNode::Leaf(concrete_leaf)
                }
                ScrapingNode::Brackets(sub_brackets) => ConcreteScrapingNode::Brackets(
                    concretize_brackets(sub_brackets, headers, globals)?,
                ),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ConcreteScrapingBrackets {
        selection_expr,
        nodes,
    })
}

fn concretize_scraper(
    scraper: Vec<ScrapingBrackets>,
    headers: &ByteRecord,
    globals: Option<&GlobalVariables>,
) -> Result<ConcreteScraper, ConcretizationError> {
    scraper
        .into_iter()
        .map(|brackets| concretize_brackets(brackets, headers, globals))
        .collect::<Result<Vec<_>, _>>()
        .map(ConcreteScraper)
}

// #[derive(Debug)]
// pub struct ScrapingProgram {}

// impl ScrapingProgram {
//     pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
//         let scraper =
//             parse_scraper(code).map_err(|_| ConcretizationError::ParseError(code.to_string()))?;

//         Ok(Self {})
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concretize_scraper() {
        let concrete_scraper = concretize_scraper(
            parse_scraper(
                "h2 > a {
                    title: text;

                    one('.main').all('p') {
                        a {
                            url: attr('href');
                        }

                        content: text;
                    }
                }",
            )
            .unwrap(),
            &ByteRecord::new(),
            None,
        )
        .unwrap();

        // dbg!(&concrete_scraper);

        assert_eq!(
            concrete_scraper.names().collect::<Vec<_>>(),
            vec!["title", "url", "content"]
        );
    }

    #[test]
    fn test_evaluate_scraper() {
        let concrete_scraper = concretize_scraper(
            parse_scraper("li {content: text;}").unwrap(),
            &ByteRecord::new(),
            None,
        )
        .unwrap();

        let html = Html::parse_fragment("<ul><li>one</li><li>two</li><li>three</li></ul>");
        let selection = Selection::Singular(html.root_element().id());

        dbg!(concrete_scraper.evaluate(&html, &selection));
    }
}
