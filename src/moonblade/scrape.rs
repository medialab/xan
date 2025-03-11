use std::sync::Arc;

use csv::ByteRecord;
use ego_tree::NodeId;
use scraper::{ElementRef, Html, Selector};

use super::error::{ConcretizationError, SpecifiedEvaluationError};
use super::interpreter::{concretize_expression, ConcreteExpr, EvaluationContext, GlobalVariables};
use super::parser::{parse_scraper, Expr, ScrapingBrackets, ScrapingNode};
use super::types::{Arity, DynamicValue};

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

impl ConcreteScrapingLeaf {
    fn evaluate(
        &self,
        index: Option<usize>,
        record: &ByteRecord,
        context: &EvaluationContext,
        html: &Html,
        selection: &Selection,
    ) -> Result<DynamicValue, SpecifiedEvaluationError> {
        let value = DynamicValue::from(self.extractor.run(html, selection));

        match &self.processing {
            None => Ok(value),
            Some(expr) => {
                // NOTE: the `GlobalVariables` lives on the stack, for now
                let mut globals = GlobalVariables::of("value");
                globals.set_value(0, value);

                expr.evaluate(index, record, context, Some(&globals), None)
            }
        }
    }
}

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
    fn evaluate(
        &self,
        index: Option<usize>,
        record: &ByteRecord,
        context: &EvaluationContext,
        html: &Html,
        selection: &Selection,
    ) -> Result<Vec<DynamicValue>, SpecifiedEvaluationError> {
        let mut values = Vec::new();

        match self {
            Self::Leaf(leaf) => {
                values.push(leaf.evaluate(index, record, context, html, selection)?);
            }
            Self::Brackets(brackets) => {
                for node in brackets.nodes.iter() {
                    values.extend(node.evaluate(index, record, context, html, selection)?);
                }
            }
        };

        Ok(values)
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

    fn evaluate(
        &self,
        index: Option<usize>,
        record: &ByteRecord,
        context: &EvaluationContext,
        html: &Html,
        selection: &Selection,
    ) -> Result<Vec<DynamicValue>, SpecifiedEvaluationError> {
        let selection = self.selection_expr.evaluate(html, selection);

        let mut values = Vec::new();

        for node in self.nodes.iter() {
            values.extend(node.evaluate(index, record, context, html, &selection)?);
        }

        Ok(values)
    }
}

#[derive(Debug)]
struct ConcreteScraper(Vec<ConcreteScrapingBrackets>);

impl ConcreteScraper {
    fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().flat_map(|brackets| brackets.names())
    }

    fn evaluate(
        &self,
        index: Option<usize>,
        record: &ByteRecord,
        context: &EvaluationContext,
        html: &Html,
        selection: &Selection,
    ) -> Result<Vec<DynamicValue>, SpecifiedEvaluationError> {
        let mut values = Vec::new();

        for brackets in self.0.iter() {
            values.extend(brackets.evaluate(index, record, context, html, selection)?);
        }

        Ok(values)
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

#[derive(Debug)]
pub struct ScrapingProgram {
    scraper: ConcreteScraper,
    context: EvaluationContext,
}

impl ScrapingProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let scraper =
            parse_scraper(code).map_err(|_| ConcretizationError::ParseError(code.to_string()))?;

        let concrete_scraper =
            concretize_scraper(scraper, headers, Some(&GlobalVariables::of("value")))?;

        Ok(Self {
            context: EvaluationContext::new(headers),
            scraper: concrete_scraper,
        })
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.scraper.names()
    }

    pub fn run(
        &self,
        index: usize,
        record: &ByteRecord,
        html: &Html,
    ) -> Result<Vec<DynamicValue>, SpecifiedEvaluationError> {
        let selection = Selection::Singular(html.root_element().id());

        self.scraper
            .evaluate(Some(index), record, &self.context, html, &selection)
    }
}

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
    fn test_scraping_program() {
        let program = ScrapingProgram::parse(
            "li {content: text; upper_content: text, upper; first: text(), value[0];}",
            &csv::ByteRecord::new(),
        )
        .unwrap();

        let html = Html::parse_fragment("<ul><li>one</li><li>two</li><li>three</li></ul>");

        assert_eq!(
            program.run(0, &csv::ByteRecord::new(), &html),
            Ok(vec![
                DynamicValue::from("one"),
                DynamicValue::from("ONE"),
                DynamicValue::from("o")
            ])
        );
    }
}
