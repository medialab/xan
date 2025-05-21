use std::convert::TryFrom;
use std::iter;
use std::sync::Arc;

use csv::ByteRecord;
use ego_tree::NodeId;
use lazy_static::lazy_static;
use regex::{Captures, Regex};
use scraper::{Element, ElementRef, Html, Node, Selector};

use crate::collections::HashMap;

use super::error::{ConcretizationError, SpecifiedEvaluationError};
use super::interpreter::{concretize_expression, ConcreteExpr, EvaluationContext, GlobalVariables};
use super::parser::{parse_scraper, Expr, ScrapingBrackets, ScrapingNode};
use super::types::{Argument, DynamicValue, FunctionArguments, HeadersIndex};

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

impl Selection {
    fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

#[derive(Debug, Clone)]
enum Pattern {
    Substring(String),
    Regex(Regex),
}

impl Pattern {
    fn is_match(&self, haystack: &str) -> bool {
        match self {
            Self::Substring(substring) => haystack.contains(substring),
            Self::Regex(regex) => regex.is_match(haystack),
        }
    }
}

#[derive(Debug, Clone)]
enum SelectionRoutine {
    Stay,
    Root,
    First(Selector, Option<Pattern>),
    Last(Selector, Option<Pattern>),
    All(Selector, Option<Pattern>),
    Parent,
    FindAncestor(Selector),
    PrevSibling,
    NextSibling,
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

            // First
            (Self::First(selector, pattern), Selection::Singular(id)) => html
                .get_element(*id)
                .select(selector)
                .find(|sub_element| match pattern {
                    Some(p) => p.is_match(&sub_element.collect_raw_text()),
                    None => true,
                })
                .map(|sub_element| Selection::Singular(sub_element.id()))
                .unwrap_or(Selection::None),

            // Last
            (Self::Last(selector, pattern), Selection::Singular(id)) => html
                .get_element(*id)
                .select(selector)
                .filter(|sub_element| match pattern {
                    Some(p) => p.is_match(&sub_element.collect_raw_text()),
                    None => true,
                })
                .last()
                .map(|sub_element| Selection::Singular(sub_element.id()))
                .unwrap_or(Selection::None),

            // All
            (Self::All(selector, pattern), Selection::Singular(id)) => {
                let element = html.get_element(*id);

                Selection::Plural(Arc::new(
                    element
                        .select(selector)
                        .filter(|e| {
                            if let Some(p) = pattern {
                                p.is_match(&e.collect_raw_text())
                            } else {
                                true
                            }
                        })
                        .map(|e| e.id())
                        .collect(),
                ))
            }

            // Parent
            (Self::Parent, Selection::Singular(id)) => html
                .get_element(*id)
                .parent_element()
                .map(|parent| Selection::Singular(parent.id()))
                .unwrap_or(Selection::None),

            // Find parent
            (Self::FindAncestor(selector), Selection::Singular(id)) => {
                let mut element = html.get_element(*id);

                while let Some(parent_element) = element.parent_element() {
                    if selector.matches(&parent_element) {
                        return Selection::Singular(parent_element.id());
                    }

                    element = parent_element;
                }

                Selection::None
            }

            // Previous sibling
            (Self::PrevSibling, Selection::Singular(id)) => html
                .get_element(*id)
                .prev_sibling_element()
                .map(|parent| Selection::Singular(parent.id()))
                .unwrap_or(Selection::None),

            // Next sibling
            (Self::NextSibling, Selection::Singular(id)) => html
                .get_element(*id)
                .next_sibling_element()
                .map(|parent| Selection::Singular(parent.id()))
                .unwrap_or(Selection::None),
        }
    }
}

#[derive(Debug, Clone)]
enum Extractor {
    Name,
    Id,
    Classes,
    RawText,
    Text,
    Json,
    JsonLd(String),
    InnerHtml,
    OuterHtml,
    Attr(String),
    Attrs,
}

impl TryFrom<Expr> for Extractor {
    type Error = ConcretizationError;

    fn try_from(value: Expr) -> Result<Self, Self::Error> {
        match value {
            Expr::Func(mut call) => Ok(match call.name.as_str() {
                "name" => Self::Name,
                "id" => Self::Id,
                "classes" => Self::Classes,
                "raw_text" => Self::RawText,
                "text" => Self::Text,
                "json" => Self::Json,
                "json_ld" => Self::JsonLd(
                    call.args
                        .pop()
                        .and_then(|(_, name)| name.try_into_string())
                        .ok_or(ConcretizationError::NotStaticallyAnalyzable)?,
                ),
                "inner_html" => Self::InnerHtml,
                "outer_html" => Self::OuterHtml,
                "attr" => Self::Attr(
                    call.args
                        .pop()
                        .and_then(|(_, name)| name.try_into_string())
                        .ok_or(ConcretizationError::NotStaticallyAnalyzable)?,
                ),
                "data" => Self::Attr(
                    call.args
                        .pop()
                        .and_then(|(_, name)| {
                            name.try_into_string().map(|s| "data-".to_string() + &s)
                        })
                        .ok_or(ConcretizationError::NotStaticallyAnalyzable)?,
                ),
                "attrs" => Self::Attrs,
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
                    Self::Name => Some(DynamicValue::from(element.value().name())),
                    Self::Id => Some(DynamicValue::from(element.value().id())),
                    Self::Classes => {
                        let classes = element
                            .value()
                            .classes()
                            .map(DynamicValue::from)
                            .collect::<Vec<_>>();

                        Some(DynamicValue::from(classes))
                    }
                    Self::RawText => Some(DynamicValue::from(element.collect_raw_text())),
                    Self::Text => Some(DynamicValue::from(element.collect_text())),
                    Self::InnerHtml => Some(DynamicValue::from(element.inner_html())),
                    Self::OuterHtml => Some(DynamicValue::from(element.html())),
                    Self::Json => Some(
                        serde_json::from_str::<DynamicValue>(&element.collect_text())
                            .unwrap_or(DynamicValue::None),
                    ),
                    Self::JsonLd(target_type) => {
                        fn compare_json_ld_types(first: &str, second: &str) -> bool {
                            let first = first
                                .strip_prefix("http://schema.org/")
                                .unwrap_or(first)
                                .to_lowercase();
                            let second = second
                                .strip_prefix("http://schema.org/")
                                .unwrap_or(second)
                                .to_lowercase();

                            first == second
                        }

                        let value = serde_json::from_str::<DynamicValue>(
                            &html_escape::decode_html_entities(&element.collect_text()),
                        )
                        .unwrap_or(DynamicValue::None);

                        let mut found = DynamicValue::None;

                        match &value {
                            DynamicValue::Map(map) => {
                                if let Some(v) = map.get("@type") {
                                    if let Ok(t) = v.try_as_str() {
                                        if compare_json_ld_types(&t, target_type) {
                                            found = value;
                                        }
                                    }
                                }
                            }
                            DynamicValue::List(list) => {
                                for item in list.iter() {
                                    if let DynamicValue::Map(map) = item {
                                        if let Some(v) = map.get("@type") {
                                            if let Ok(t) = v.try_as_str() {
                                                if compare_json_ld_types(&t, target_type) {
                                                    found = item.clone();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            _ => (),
                        };

                        Some(found)
                    }
                    Self::Attr(name) => element.attr(name).map(DynamicValue::from),
                    Self::Attrs => {
                        let mut map: HashMap<String, DynamicValue> =
                            HashMap::with_capacity(element.value().attrs.len());

                        for (name, value) in element.value().attrs() {
                            map.insert(name.to_string(), DynamicValue::from(value));
                        }

                        Some(DynamicValue::from(map))
                    }
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
        context: &EvaluationContext,
        html: &Html,
        selection: &Selection,
    ) -> Result<DynamicValue, SpecifiedEvaluationError> {
        if selection.is_none() {
            return Ok(DynamicValue::None);
        }

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

                expr.evaluate(&context.with_globals(&globals))
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
        context: &EvaluationContext,
        html: &Html,
        selection: &Selection,
    ) -> Result<(), SpecifiedEvaluationError> {
        match self {
            Self::Leaf(leaf) => {
                scratch.push(leaf.evaluate(context, html, selection)?);
            }
            Self::Brackets(brackets) => {
                brackets.evaluate(scratch, context, html, selection)?;
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
        context: &EvaluationContext,
        html: &Html,
        selection: &Selection,
    ) -> Result<(), SpecifiedEvaluationError> {
        let selection = self.selection_expr.evaluate(html, selection);

        for node in self.nodes.iter() {
            node.evaluate(scratch, context, html, &selection)?;
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
        context: &EvaluationContext,
        html: &Html,
        selection: &Selection,
    ) -> Result<(), SpecifiedEvaluationError> {
        for brackets in self.0.iter() {
            brackets.evaluate(scratch, context, html, selection)?;
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

fn parse_contains_pattern(expr: Expr) -> Result<Pattern, ConcretizationError> {
    let concrete_expr = concretize_expression(expr, &csv::ByteRecord::new(), None)?;
    let value = concrete_expr.try_unwrap()?;

    if let DynamicValue::Regex(regex) = value {
        return Ok(Pattern::Regex((*regex).clone()));
    }

    let substring = value
        .try_as_str()
        .map_err(|_| ConcretizationError::NotStaticallyAnalyzable)?;

    Ok(Pattern::Substring(substring.into_owned()))
}

fn get_selection_function_arguments(name: &str) -> Option<FunctionArguments> {
    Some(match name {
        "stay" | "root" => FunctionArguments::nullary(),
        "parent" | "prev_sibling" | "next_sibling" => FunctionArguments::with_range(0..=1),
        "find_ancestor" => FunctionArguments::with_range(1..=2),
        "first" | "all" | "last" => FunctionArguments::complex(vec![
            Argument::Positional,
            Argument::Optional,
            Argument::with_name("containing"),
        ]),
        _ => return None,
    })
}

fn concretize_selection_expr(
    expr: Expr,
    headers: &ByteRecord,
    globals: Option<&GlobalVariables>,
) -> Result<ConcreteSelectionExpr, ConcretizationError> {
    match expr {
        Expr::Func(call) => {
            let function_arguments = get_selection_function_arguments(&call.name)
                .ok_or_else(|| ConcretizationError::UnknownFunction(call.name.to_string()))?;

            function_arguments
                .validate_arity(call.args.len())
                .map_err(|invalid_arity| {
                    ConcretizationError::InvalidArity(call.name.to_string(), invalid_arity)
                })?;

            // Nullary
            if call.name == "stay" {
                return Ok(ConcreteSelectionExpr::Call(SelectionRoutine::Stay, vec![]));
            } else if call.name == "root" {
                return Ok(ConcreteSelectionExpr::Call(SelectionRoutine::Root, vec![]));
            }

            let (mut positionals, named): (Vec<_>, Vec<_>) =
                call.args.into_iter().partition(|arg| arg.0.is_none());

            // Unary
            if ["parent", "prev_sibling", "next_sibling"].contains(&call.name.as_str()) {
                let target = if positionals.is_empty() {
                    ConcreteSelectionExpr::Call(SelectionRoutine::Stay, vec![])
                } else {
                    concretize_selection_expr(positionals.pop().unwrap().1, headers, globals)?
                };

                return Ok(ConcreteSelectionExpr::Call(
                    match call.name.as_str() {
                        "parent" => SelectionRoutine::Parent,
                        "prev_sibling" => SelectionRoutine::PrevSibling,
                        "next_sibling" => SelectionRoutine::NextSibling,
                        _ => unreachable!(),
                    },
                    vec![target],
                ));
            }

            // Binary?
            let arg = positionals.pop().unwrap().1;
            let concrete_arg = concretize_expression(arg, headers, globals)?;

            let args = if positionals.is_empty() {
                vec![]
            } else {
                vec![concretize_selection_expr(
                    positionals.pop().unwrap().1,
                    headers,
                    globals,
                )?]
            };

            // TODO: can probably be factorized lol.
            let selection_expr = match call.name.as_str() {
                "first" => {
                    let selector = parse_selector(concrete_arg)?;
                    let pattern = named
                        .into_iter()
                        .find(|arg| matches!(&arg.0, Some(name) if name == "containing"))
                        .map(|(_, arg)| parse_contains_pattern(arg))
                        .transpose()?;
                    ConcreteSelectionExpr::Call(SelectionRoutine::First(selector, pattern), args)
                }
                "last" => {
                    let selector = parse_selector(concrete_arg)?;
                    let pattern = named
                        .into_iter()
                        .find(|arg| matches!(&arg.0, Some(name) if name == "containing"))
                        .map(|(_, arg)| parse_contains_pattern(arg))
                        .transpose()?;
                    ConcreteSelectionExpr::Call(SelectionRoutine::Last(selector, pattern), args)
                }
                "all" => {
                    let selector = parse_selector(concrete_arg)?;
                    let pattern = named
                        .into_iter()
                        .find(|arg| matches!(&arg.0, Some(name) if name == "containing"))
                        .map(|(_, arg)| parse_contains_pattern(arg))
                        .transpose()?;
                    ConcreteSelectionExpr::Call(SelectionRoutine::All(selector, pattern), args)
                }
                "find_ancestor" => {
                    let selector = parse_selector(concrete_arg)?;
                    ConcreteSelectionExpr::Call(SelectionRoutine::FindAncestor(selector), args)
                }
                _ => return Err(ConcretizationError::UnknownFunction(call.name.to_string())),
            };

            Ok(selection_expr)
        }
        Expr::Str(css) => {
            let selector = Selector::parse(&css)
                .map_err(|_| ConcretizationError::InvalidCSSSelector(css.to_string()))?;

            Ok(ConcreteSelectionExpr::Call(
                SelectionRoutine::First(selector, None),
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
    headers_index: HeadersIndex,
    pub capacity: usize,
}

impl ScrapingProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let scraper = parse_scraper(code).map_err(ConcretizationError::ParseError)?;

        let concrete_scraper =
            concretize_scraper(scraper, headers, Some(&GlobalVariables::of("value")))?;

        let capacity = concrete_scraper.names().count();

        Ok(Self {
            headers_index: HeadersIndex::from_headers(headers),
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

        let context = EvaluationContext {
            index: Some(index),
            record,
            headers_index: &self.headers_index,
            globals: None,
            lambda_variables: None,
        };

        self.scraper
            .evaluate(&mut scratch, &context, html, &selection)?;

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

            let context = EvaluationContext {
                index: Some(index),
                record,
                headers_index: &self.headers_index,
                globals: None,
                lambda_variables: None,
            };

            self.scraper
                .evaluate(&mut scratch, &context, html, &selection)?;

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

                    first('.main').all('p') {
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
