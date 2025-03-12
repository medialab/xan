use std::convert::TryFrom;
use std::iter;
use std::sync::Arc;

use csv::ByteRecord;
use ego_tree::NodeId;
use lazy_static::lazy_static;
use regex::{Captures, Regex};
use scraper::{ElementRef, Html, Node, Selector};

use super::error::{ConcretizationError, SpecifiedEvaluationError};
use super::interpreter::{concretize_expression, ConcreteExpr, EvaluationContext, GlobalVariables};
use super::parser::{parse_scraper, Expr, ScrapingBrackets, ScrapingNode};
use super::types::{Arity, DynamicValue};

trait HtmlExt {
    fn get_element(&self, id: NodeId) -> ElementRef;
}

impl HtmlExt for Html {
    fn get_element(&self, id: NodeId) -> ElementRef {
        ElementRef::wrap(self.tree.get(id).unwrap()).unwrap()
    }
}

lazy_static! {
    static ref SQUEEZE_REGEX: Regex = Regex::new(r"\s+").unwrap();
    static ref WHITESPACE_SQUEEZE_REGEX: Regex = Regex::new(r" +").unwrap();
    static ref PARAGRAPH_NORMALIZER_REGEX: Regex = Regex::new(r"\n{3,}").unwrap();
    static ref INDENTATION_NORMALIZER_REGEX: Regex = Regex::new(r"(?m)^ +(.*) *$").unwrap();
    static ref BLOCK_ELEMENT_REGEX: Regex = Regex::new(r"(?i)^(?:article|aside|blockquote|body|button|canvas|caption|col|colgroup|dd|div|dl|dt|embed|fieldset|figcaption|figure|footer|form|h1|h2|h3|h4|h5|h6|header|hgroup|li|map|object|ol|output|p|pre|progress|section|table|tbody|textarea|tfoot|thead|tr|ul|video)$").unwrap();
}

trait ElementRefExt {
    fn collect_raw_text(&self) -> String;
    fn collect_text(&self) -> String;
}

fn collect_text_inner(scratch: &mut String, element: &ElementRef, squeeze: bool) {
    for child in element.children() {
        match child.value() {
            Node::Element(node) => {
                if node.name() == "br" {
                    scratch.push('\n');
                    continue;
                } else if node.name() == "hr" {
                    scratch.push_str("\n\n");
                    continue;
                }

                let is_block = BLOCK_ELEMENT_REGEX.is_match(node.name());

                if is_block {
                    scratch.push_str("\n\n");
                }

                collect_text_inner(
                    scratch,
                    &ElementRef::wrap(child).unwrap(),
                    node.name() != "pre",
                );

                if is_block {
                    scratch.push_str("\n\n");
                }
            }
            Node::Text(text) => {
                if squeeze {
                    scratch.push_str(&SQUEEZE_REGEX.replace_all(text, " "));
                } else {
                    scratch.push_str(text);
                }
            }
            _ => continue,
        }
    }
}

impl ElementRefExt for ElementRef<'_> {
    fn collect_raw_text(&self) -> String {
        let mut string = String::new();

        for (i, text) in self.text().enumerate() {
            if i == 0 {
                string.push_str(text.trim_start());
            } else {
                string.push_str(text);
            }
        }

        string.truncate(string.trim_end().len());

        string
    }

    fn collect_text(&self) -> String {
        let mut string = String::new();

        collect_text_inner(&mut string, self, self.value().name() != "pre");

        let string = WHITESPACE_SQUEEZE_REGEX.replace_all(string.trim(), " ");
        let string = INDENTATION_NORMALIZER_REGEX
            .replace_all(&string, |caps: &Captures| caps[1].to_string());
        let string = PARAGRAPH_NORMALIZER_REGEX.replace_all(&string, "\n\n");

        string.into_owned()
    }
}

#[derive(Debug, Clone)]
enum Selection {
    None,
    Singular(NodeId),
    Plural(Arc<Vec<NodeId>>),
}

#[derive(Debug, Clone)]
enum SelectionRoutine {
    Stay,
    Root,
    One(Selector),
    All(Selector),
    Contains(String),
}

impl SelectionRoutine {
    fn run(&self, html: &Html, selection: &Selection) -> Selection {
        match (self, selection) {
            // Staying where we are
            (_, Selection::None) | (Self::Stay, _) => selection.clone(),

            // Up to the root
            (Self::Root, _) => Selection::Singular(html.root_element().id()),

            // Plural selection is always a matter of flatmap
            (_, Selection::Plural(ids)) => {
                let new_ids = ids
                    .iter()
                    .flat_map(|id| -> Box<dyn Iterator<Item = NodeId>> {
                        let sub_selection = Selection::Singular(*id);

                        match self.run(html, &sub_selection) {
                            Selection::None => Box::new(iter::empty()),
                            Selection::Singular(id) => Box::new(iter::once(id)),
                            Selection::Plural(ids) => {
                                Box::new(ids.iter().copied().collect::<Vec<_>>().into_iter())
                            }
                        }
                    })
                    .collect::<Vec<_>>();

                Selection::Plural(Arc::new(new_ids))
            }

            // One
            (Self::One(selector), Selection::Singular(id)) => {
                let element = html.get_element(*id);

                element
                    .select(selector)
                    .next()
                    .map(|e| Selection::Singular(e.id()))
                    .unwrap_or_else(|| Selection::None)
            }

            // All
            (Self::All(selector), Selection::Singular(id)) => {
                let element = html.get_element(*id);

                Selection::Plural(Arc::new(element.select(selector).map(|e| e.id()).collect()))
            }

            // Contains
            (Self::Contains(pattern), Selection::Singular(id)) => {
                let element = html.get_element(*id);

                if element.collect_raw_text().contains(pattern) {
                    selection.clone()
                } else {
                    Selection::None
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
enum Extractor {
    RawText,
    Text,
    Json,
    InnerHtml,
    OuterHtml,
    Attr(String),
}

impl TryFrom<Expr> for Extractor {
    type Error = ConcretizationError;

    fn try_from(value: Expr) -> Result<Self, Self::Error> {
        match value {
            Expr::Func(mut call) => Ok(match call.name.as_str() {
                "raw_text" => Self::RawText,
                "text" => Self::Text,
                "json" => Self::Json,
                "inner_html" => Self::InnerHtml,
                "outer_html" => Self::OuterHtml,
                "attr" => Self::Attr(
                    call.args
                        .pop()
                        .and_then(|(_, name)| name.try_into_string())
                        .ok_or(ConcretizationError::NotStaticallyAnalyzable)?,
                ),
                _ => return Err(ConcretizationError::UnknownFunction(call.name)),
            }),
            _ => Err(ConcretizationError::NotStaticallyAnalyzable),
        }
    }
}

impl Extractor {
    fn run(&self, html: &Html, selection: &Selection) -> Option<DynamicValue> {
        match selection {
            Selection::None => None,
            Selection::Singular(id) => {
                let element = html.get_element(*id);

                match self {
                    Self::RawText => Some(DynamicValue::from(element.collect_raw_text())),
                    Self::Text => Some(DynamicValue::from(element.collect_text())),
                    Self::InnerHtml => Some(DynamicValue::from(element.inner_html())),
                    Self::OuterHtml => Some(DynamicValue::from(element.html())),
                    Self::Json => Some(
                        serde_json::from_str::<DynamicValue>(&element.collect_text())
                            .unwrap_or(DynamicValue::None),
                    ),
                    Self::Attr(name) => element.attr(name).map(DynamicValue::from),
                }
            }
            Selection::Plural(ids) => Some(DynamicValue::from(
                ids.iter()
                    .flat_map(|id| {
                        let sub_selection = Selection::Singular(*id);
                        self.run(html, &sub_selection)
                    })
                    .collect::<Vec<_>>(),
            )),
        }
    }
}

// NOTE: this is an enum because it will be easier to extend later on if we
// need to complexify evaluation of the selection part.
#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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
        let value = self
            .extractor
            .run(html, selection)
            .unwrap_or(DynamicValue::None);

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

#[derive(Debug, Clone)]
enum ConcreteScrapingNode {
    Brackets(ConcreteScrapingBrackets),
    Leaf(ConcreteScrapingLeaf),
}

impl ConcreteScrapingNode {
    fn names(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        match self {
            Self::Leaf(leaf) => Box::new(iter::once(leaf.name.as_str())),
            Self::Brackets(brackets) => {
                Box::new(brackets.nodes.iter().flat_map(|node| node.names()))
            }
        }
    }

    fn evaluate(
        &self,
        scratch: &mut Vec<DynamicValue>,
        index: Option<usize>,
        record: &ByteRecord,
        context: &EvaluationContext,
        html: &Html,
        selection: &Selection,
    ) -> Result<(), SpecifiedEvaluationError> {
        match self {
            Self::Leaf(leaf) => {
                scratch.push(leaf.evaluate(index, record, context, html, selection)?);
            }
            Self::Brackets(brackets) => {
                brackets.evaluate(scratch, index, record, context, html, selection)?;
            }
        };

        Ok(())
    }
}

#[derive(Debug, Clone)]
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
        scratch: &mut Vec<DynamicValue>,
        index: Option<usize>,
        record: &ByteRecord,
        context: &EvaluationContext,
        html: &Html,
        selection: &Selection,
    ) -> Result<(), SpecifiedEvaluationError> {
        let selection = self.selection_expr.evaluate(html, selection);

        for node in self.nodes.iter() {
            node.evaluate(scratch, index, record, context, html, &selection)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ConcreteScraper(Vec<ConcreteScrapingBrackets>);

impl ConcreteScraper {
    fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().flat_map(|brackets| brackets.names())
    }

    fn evaluate(
        &self,
        scratch: &mut Vec<DynamicValue>,
        index: Option<usize>,
        record: &ByteRecord,
        context: &EvaluationContext,
        html: &Html,
        selection: &Selection,
    ) -> Result<(), SpecifiedEvaluationError> {
        for brackets in self.0.iter() {
            brackets.evaluate(scratch, index, record, context, html, selection)?;
        }

        Ok(())
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
            if call.name == "stay" {
                return Ok(ConcreteSelectionExpr::Call(SelectionRoutine::Stay, vec![]));
            } else if call.name == "root" {
                return Ok(ConcreteSelectionExpr::Call(SelectionRoutine::Root, vec![]));
            }

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
                "contains" => ConcreteSelectionExpr::Call(
                    SelectionRoutine::Contains(
                        concrete_arg
                            .try_unwrap()?
                            .try_as_str()
                            .unwrap()
                            .into_owned(),
                    ),
                    args,
                ),
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
                        extractor: Extractor::try_from(leaf.expr)?,
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

#[derive(Debug, Clone)]
pub struct ScrapingProgram {
    scraper: ConcreteScraper,
    context: EvaluationContext,
    capacity: usize,
}

impl ScrapingProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let scraper =
            parse_scraper(code).map_err(|_| ConcretizationError::ParseError(code.to_string()))?;

        let concrete_scraper =
            concretize_scraper(scraper, headers, Some(&GlobalVariables::of("value")))?;

        let capacity = concrete_scraper.names().count();

        Ok(Self {
            context: EvaluationContext::new(headers),
            scraper: concrete_scraper,
            capacity,
        })
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.scraper.names()
    }

    pub fn run_singular(
        &self,
        index: usize,
        record: &ByteRecord,
        html: &Html,
    ) -> Result<Vec<DynamicValue>, SpecifiedEvaluationError> {
        let selection = Selection::Singular(html.root_element().id());
        let mut scratch = Vec::with_capacity(self.capacity);

        self.scraper.evaluate(
            &mut scratch,
            Some(index),
            record,
            &self.context,
            html,
            &selection,
        )?;

        Ok(scratch)
    }

    pub fn run_plural<'a>(
        &'a self,
        index: usize,
        record: &'a ByteRecord,
        html: &'a Html,
        selector: &'a Selector,
    ) -> impl Iterator<Item = Result<Vec<DynamicValue>, SpecifiedEvaluationError>> + 'a {
        html.select(selector).map(move |element| {
            let selection = Selection::Singular(element.id());
            let mut scratch = Vec::with_capacity(self.capacity);

            self.scraper.evaluate(
                &mut scratch,
                Some(index),
                record,
                &self.context,
                html,
                &selection,
            )?;

            Ok(scratch)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(html: &str, code: &str) -> Result<Vec<DynamicValue>, SpecifiedEvaluationError> {
        let html = Html::parse_document(html);
        let program = ScrapingProgram::parse(code, &csv::ByteRecord::new()).unwrap();
        program.run_singular(0, &csv::ByteRecord::new(), &html)
    }

    #[test]
    fn test_collect_raw_text() {
        let html = Html::parse_document("<ul><li>one</li><li>two</li><li>three</li></ul>");
        assert_eq!(html.root_element().collect_raw_text(), "onetwothree");
    }

    #[test]
    fn test_collect_text() {
        let html = Html::parse_document("<ul><li>one</li><li>two</li><li>three</li></ul>");
        assert_eq!(html.root_element().collect_text(), "one\n\ntwo\n\nthree");

        let the_worst_html =
            Html::parse_document(include_str!("../../tests/resources/the_worst.html"));

        dbg!(the_worst_html.root_element().collect_text());
    }

    #[test]
    fn test_concretization() {
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
    fn test_basics() {
        assert_eq!(
            eval(
                "<ul><li>one</li><li>two</li><li>three</li></ul>",
                "li {content: text; upper_content: text, upper; first: text(), value[0];}"
            ),
            Ok(vec![
                DynamicValue::from("one"),
                DynamicValue::from("ONE"),
                DynamicValue::from("o")
            ])
        );
    }

    #[test]
    fn test_all() {
        assert_eq!(
            eval(
                "<ul><li>one</li><li>two</li><li>three</li></ul>",
                "all('li') {text: text}"
            ),
            Ok(vec![DynamicValue::from(vec![
                DynamicValue::from("one"),
                DynamicValue::from("two"),
                DynamicValue::from("three")
            ])])
        );
    }
}
