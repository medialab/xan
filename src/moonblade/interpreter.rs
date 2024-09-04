use std::collections::HashMap;
use std::fmt;

use csv::ByteRecord;
use regex::RegexBuilder;

use super::error::{ConcretizationError, EvaluationError, SpecifiedEvaluationError};
use super::functions::{get_function, Function};
use super::parser::{parse_expression, Expr, FunctionCall};
use super::special_functions::{get_special_function, RuntimeFunction as SpecialFunction};
use super::types::{
    BoundArguments, ColumIndexationBy, DynamicValue, EvaluationResult, FunctionArguments,
    HeadersIndex, BOUND_ARGUMENTS_CAPACITY,
};

#[derive(Debug, Clone, Default)]
pub struct EvaluationContext {
    headers_index: HeadersIndex,
}

impl EvaluationContext {
    pub fn new(headers: &ByteRecord) -> Self {
        Self {
            headers_index: HeadersIndex::from_headers(headers),
        }
    }

    pub fn get_column_index(&self, indexation: &ColumIndexationBy) -> Option<usize> {
        self.headers_index.get(indexation)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConcreteExpr {
    Column(usize),
    Value(DynamicValue),
    List(Vec<ConcreteExpr>),
    Map(Vec<(String, ConcreteExpr)>),
    Call(ConcreteFunctionCall),
    SpecialCall(ConcreteSpecialFunctionCall),
}

// NOTE: the bind/evaluate distinction is still useful to propagate the calling
// function context when constructing specified errors.
impl ConcreteExpr {
    fn is_value(&self) -> bool {
        matches!(self, Self::Value(_))
    }

    fn as_column(&self) -> Option<usize> {
        match self {
            Self::Column(index) => Some(*index),
            _ => None,
        }
    }

    fn unwrap(self) -> DynamicValue {
        match self {
            Self::Value(v) => v,
            _ => panic!("cannot unwrap"),
        }
    }

    fn bind(&self, record: &ByteRecord) -> Result<DynamicValue, EvaluationError> {
        Ok(match self {
            Self::Value(value) => value.clone(),
            Self::Column(index) => match record.get(*index) {
                None => return Err(EvaluationError::ColumnOutOfRange(*index)),
                Some(cell) => match std::str::from_utf8(cell) {
                    Err(_) => return Err(EvaluationError::UnicodeDecodeError),
                    Ok(value) => DynamicValue::from(value),
                },
            },
            Self::List(_) | Self::Map(_) | Self::Call(_) | Self::SpecialCall(_) => unreachable!(),
        })
    }

    pub fn evaluate<'a>(
        &'a self,
        index: Option<usize>,
        record: &ByteRecord,
        context: &'a EvaluationContext,
    ) -> EvaluationResult {
        match self {
            Self::Call(function_call) => function_call.run(index, record, context),
            Self::SpecialCall(function_call) => function_call.run(index, record, context),
            Self::List(items) => {
                let mut bound = Vec::with_capacity(items.len());

                for item in items {
                    bound.push(item.evaluate(index, record, context)?);
                }

                Ok(DynamicValue::from(bound))
            }
            Self::Map(pairs) => {
                let mut bound = HashMap::with_capacity(pairs.len());

                for (k, v) in pairs {
                    bound.insert(k.to_string(), v.evaluate(index, record, context)?);
                }

                Ok(DynamicValue::from(bound))
            }
            _ => self.bind(record).map_err(|err| SpecifiedEvaluationError {
                function_name: "<expr>".to_string(),
                reason: err,
            }),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct ConcreteFunctionCall {
    name: String,
    function: Function,
    args: Vec<ConcreteExpr>,
}

impl ConcreteFunctionCall {
    fn is_statically_evaluable(&self) -> bool {
        // NOTE: nullary functions such as index() or uuid() usually
        // rely on some external implicit state and cannot be statically
        // evaluated.
        !self.args.is_empty() && self.args.iter().all(|arg| arg.is_value())
    }

    fn static_run(&self) -> EvaluationResult {
        self.run(None, &ByteRecord::new(), &EvaluationContext::default())
    }

    fn run<'a>(
        &'a self,
        index: Option<usize>,
        record: &ByteRecord,
        context: &'a EvaluationContext,
    ) -> EvaluationResult {
        let mut bound_args = BoundArguments::new();

        for arg in self.args.iter() {
            match arg {
                ConcreteExpr::Call(sub_function_call) => {
                    bound_args.push(sub_function_call.run(index, record, context)?);
                }
                ConcreteExpr::SpecialCall(sub_function_call) => {
                    bound_args.push(sub_function_call.run(index, record, context)?);
                }
                ConcreteExpr::List(_) | ConcreteExpr::Map(_) => {
                    bound_args.push(arg.evaluate(index, record, context)?)
                }
                _ => bound_args.push(arg.bind(record).map_err(|err| SpecifiedEvaluationError {
                    function_name: self.name.to_string(),
                    reason: err,
                })?),
            }
        }

        match (self.function)(bound_args) {
            Ok(value) => Ok(value),
            Err(err) => Err(SpecifiedEvaluationError {
                function_name: self.name.clone(),
                reason: err,
            }),
        }
    }
}

// NOTE: in older rust versions, Debug cannot be derived
// correctly from `fn` and it will not compile without
// this custom `Debug` implementation
impl fmt::Debug for ConcreteFunctionCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConcreteFunctionCall")
            .field("name", &self.name)
            .field("args", &self.args)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct ConcreteSpecialFunctionCall {
    name: String,
    function: SpecialFunction,
    args: Vec<ConcreteExpr>,
}

impl ConcreteSpecialFunctionCall {
    fn is_statically_evaluable(&self) -> bool {
        // NOTE: other special function are not suitable for late
        // statical evaluation.
        if self.name != "if" && self.name != "unless" {
            return false;
        }

        self.args.iter().all(|arg| arg.is_value())
    }

    fn static_run(&self) -> EvaluationResult {
        self.run(None, &ByteRecord::new(), &EvaluationContext::default())
    }

    fn run<'a>(
        &'a self,
        index: Option<usize>,
        record: &ByteRecord,
        context: &'a EvaluationContext,
    ) -> EvaluationResult {
        (self.function)(index, record, context, &self.args)
    }
}

// NOTE: in older rust versions, Debug cannot be derived
// correctly from `fn` and it will not compile without
// this custom `Debug` implementation
impl fmt::Debug for ConcreteSpecialFunctionCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConcreteSpecialFunctionCall")
            .field("name", &self.name)
            .field("args", &self.args)
            .finish()
    }
}

fn concretize_arguments(
    function_arguments: &FunctionArguments,
    parsed_args: Vec<(Option<String>, Expr)>,
    headers: &ByteRecord,
) -> Result<Vec<ConcreteExpr>, ConcretizationError> {
    let concrete_args = parsed_args
        .into_iter()
        .map(|(name, expr)| concretize_expression(expr, headers).map(|r| (name, r)))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(function_arguments
        .reorder(concrete_args)?
        .into_iter()
        .map(|opt| opt.unwrap_or(ConcreteExpr::Value(DynamicValue::None)))
        .collect())
}

fn concretize_call(
    call: FunctionCall,
    headers: &ByteRecord,
) -> Result<ConcreteExpr, ConcretizationError> {
    let function_name = &call.name;
    let actual_arity = call.args.len();

    if actual_arity > BOUND_ARGUMENTS_CAPACITY {
        return Err(ConcretizationError::TooManyArguments(actual_arity));
    }

    // Dealing with special functions
    if let Some((comptime_function, runtime_function, arguments)) =
        get_special_function(function_name)
    {
        arguments
            .validate_arity(actual_arity)
            .map_err(|invalid_arity| {
                ConcretizationError::InvalidArity(function_name.clone(), invalid_arity)
            })?;

        // Some function can be evaluated when concretizing if they
        // are statically analyzable.
        if let Some(function) = comptime_function {
            // NOTE: some function must be statically analyzable and will
            // yell if they don't have a runtime counterpart.
            if let Some(concrete_expr) = function(&call, headers)? {
                return Ok(concrete_expr);
            }
        }

        let concrete_call = ConcreteSpecialFunctionCall {
            name: function_name.clone(),
            function: runtime_function.expect("missing special function runtime"),
            args: concretize_arguments(&arguments, call.args, headers)?,
        };

        if concrete_call.is_statically_evaluable() {
            match concrete_call.static_run() {
                Err(evaluation_error) => {
                    return Err(ConcretizationError::StaticEvaluationError(evaluation_error))
                }
                Ok(value) => return Ok(ConcreteExpr::Value(value)),
            };
        }

        return Ok(ConcreteExpr::SpecialCall(concrete_call));
    }

    Ok(match get_function(function_name) {
        None => return Err(ConcretizationError::UnknownFunction(function_name.clone())),
        Some((function, arguments)) => {
            arguments
                .validate_arity(actual_arity)
                .map_err(|invalid_arity| {
                    ConcretizationError::InvalidArity(function_name.clone(), invalid_arity)
                })?;

            let concrete_call = ConcreteFunctionCall {
                name: function_name.clone(),
                function,
                args: concretize_arguments(&arguments, call.args, headers)?,
            };

            if concrete_call.is_statically_evaluable() {
                match concrete_call.static_run() {
                    Err(evaluation_error) => {
                        return Err(ConcretizationError::StaticEvaluationError(evaluation_error))
                    }
                    Ok(value) => return Ok(ConcreteExpr::Value(value)),
                };
            }

            ConcreteExpr::Call(concrete_call)
        }
    })
}

fn concretize_list(
    list: Vec<Expr>,
    headers: &ByteRecord,
) -> Result<ConcreteExpr, ConcretizationError> {
    let concrete_list = list
        .into_iter()
        .map(|item| concretize_expression(item, headers))
        .collect::<Result<Vec<ConcreteExpr>, _>>()?;

    // NOTE: here we can collapse to a literal value
    Ok(if concrete_list.iter().all(|e| e.is_value()) {
        ConcreteExpr::Value(DynamicValue::from(
            concrete_list
                .into_iter()
                .map(|e| e.unwrap())
                .collect::<Vec<_>>(),
        ))
    } else {
        ConcreteExpr::List(concrete_list)
    })
}

fn concretize_map(
    map: Vec<(String, Expr)>,
    headers: &ByteRecord,
) -> Result<ConcreteExpr, ConcretizationError> {
    let concrete_map = map
        .into_iter()
        .map(|(k, v)| concretize_expression(v, headers).map(|e| (k, e)))
        .collect::<Result<Vec<(String, ConcreteExpr)>, _>>()?;

    // NOTE: here we can collapse to a literal value
    Ok(if concrete_map.iter().all(|(_, e)| e.is_value()) {
        ConcreteExpr::Value(DynamicValue::from(
            concrete_map
                .into_iter()
                .map(|(k, e)| (k, e.unwrap()))
                .collect::<HashMap<_, _>>(),
        ))
    } else {
        ConcreteExpr::Map(concrete_map)
    })
}

pub fn concretize_expression(
    expr: Expr,
    headers: &ByteRecord,
) -> Result<ConcreteExpr, ConcretizationError> {
    Ok(match expr {
        Expr::Underscore => unreachable!(),
        Expr::Null => ConcreteExpr::Value(DynamicValue::None),
        Expr::Bool(v) => ConcreteExpr::Value(DynamicValue::Boolean(v)),
        Expr::Float(v) => ConcreteExpr::Value(DynamicValue::Float(v)),
        Expr::Int(v) => ConcreteExpr::Value(DynamicValue::Integer(v)),
        Expr::Str(v) => ConcreteExpr::Value(DynamicValue::String(v)),
        Expr::Identifier(name) => {
            let indexation = ColumIndexationBy::Name(name);

            match indexation.find_column_index(headers) {
                Some(index) => ConcreteExpr::Column(index),
                None => return Err(ConcretizationError::ColumnNotFound(indexation)),
            }
        }
        Expr::Regex(pattern, case_insensitive) => match RegexBuilder::new(&pattern)
            .case_insensitive(case_insensitive)
            .build()
        {
            Ok(regex) => ConcreteExpr::Value(DynamicValue::from(regex)),
            Err(_) => return Err(ConcretizationError::InvalidRegex(pattern)),
        },
        Expr::Func(call) => concretize_call(call, headers)?,
        Expr::List(list) => concretize_list(list, headers)?,
        Expr::Map(map) => concretize_map(map, headers)?,
        Expr::Slice(_) => unreachable!(),
    })
}

pub fn eval_expression(
    expr: &ConcreteExpr,
    index: Option<usize>,
    record: &ByteRecord,
    context: &EvaluationContext,
) -> Result<DynamicValue, SpecifiedEvaluationError> {
    expr.evaluate(index, record, context)
}

#[derive(Clone)]
pub struct Program {
    pub expr: ConcreteExpr,
    context: EvaluationContext,
}

impl Program {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let expr = match parse_expression(code) {
            Err(_) => return Err(ConcretizationError::ParseError(code.to_string())),
            Ok(parsed_expr) => concretize_expression(parsed_expr, headers)?,
        };

        Ok(Self {
            expr,
            context: EvaluationContext::new(headers),
        })
    }

    pub fn run_with_record(
        &self,
        index: usize,
        record: &ByteRecord,
    ) -> Result<DynamicValue, SpecifiedEvaluationError> {
        eval_expression(&self.expr, Some(index), record, &self.context)
    }

    pub fn generate_key(
        &self,
        index: usize,
        record: &ByteRecord,
    ) -> Result<String, SpecifiedEvaluationError> {
        if let Some(index) = self.expr.as_column() {
            Ok(String::from_utf8(record[index].to_vec()).unwrap())
        } else {
            let value = self.run_with_record(index, record)?;
            Ok(value
                .try_into_string()
                .map_err(|err| SpecifiedEvaluationError {
                    function_name: "<expr>".to_string(),
                    reason: err,
                })?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::error::RunError;
    use super::*;
    use jiff::{tz::TimeZone, Timestamp};

    type TestResult = Result<DynamicValue, RunError>;

    fn concretize_code(code: &str) -> Result<ConcreteExpr, ConcretizationError> {
        let mut headers = ByteRecord::new();
        headers.push_field(b"name");
        headers.push_field(b"surname");
        headers.push_field(b"a");
        headers.push_field(b"b");

        let program = Program::parse(code, &headers)?;

        Ok(program.expr)
    }

    fn eval_code(code: &str) -> TestResult {
        let mut headers = ByteRecord::new();
        headers.push_field(b"name");
        headers.push_field(b"surname");
        headers.push_field(b"a");
        headers.push_field(b"b");

        let program = Program::parse(code, &headers).map_err(RunError::Prepare)?;

        let mut record = ByteRecord::new();
        record.push_field(b"john");
        record.push_field(b"SMITH");
        record.push_field(b"34");
        record.push_field(b"62");

        program
            .run_with_record(2, &record)
            .map_err(RunError::Evaluation)
    }

    #[test]
    fn test_static_evaluation() {
        assert_eq!(
            concretize_code("1 + name"),
            Ok(ConcreteExpr::Call(ConcreteFunctionCall {
                name: "add".to_string(),
                function: get_function("add").unwrap().0,
                args: vec![
                    ConcreteExpr::Value(DynamicValue::Integer(1)),
                    ConcreteExpr::Column(0)
                ]
            }))
        );

        assert_eq!(
            concretize_code("1 + 2 * 4"),
            Ok(ConcreteExpr::Value(DynamicValue::Integer(9)))
        );
    }

    #[test]
    fn test_pipeline_optimization_correctness() {
        assert_eq!(
            eval_code("trim(a) | add(a, b) | trim | add(a, b) | len"),
            Ok(DynamicValue::Integer(2))
        );

        assert_eq!(
            eval_code("trim(a) | len | add(b, _)"),
            Ok(DynamicValue::Integer(64))
        );
    }

    #[test]
    fn test_index() {
        assert_eq!(eval_code("index() + 2"), Ok(DynamicValue::from(4)));
    }

    #[test]
    fn test_typeof() {
        assert_eq!(eval_code("typeof(name)"), Ok(DynamicValue::from("string")));
        assert_eq!(eval_code("TYPEOF(name)"), Ok(DynamicValue::from("string")));
    }

    #[test]
    fn test_split_join() {
        assert_eq!(
            eval_code("split(name, 'o')"),
            Ok(DynamicValue::from(vec![
                DynamicValue::from("j"),
                DynamicValue::from("hn"),
            ]))
        );

        assert_eq!(
            eval_code("split(name, 'o', 1)"),
            Ok(DynamicValue::from(vec![
                DynamicValue::from("j"),
                DynamicValue::from("hn"),
            ]))
        );

        assert_eq!(
            eval_code("split(name, 'o') | join(_, '&')"),
            Ok(DynamicValue::from("j&hn"))
        )
    }

    #[test]
    fn test_arithmetics() {
        assert_eq!(eval_code("add(1, 2)"), Ok(DynamicValue::Integer(3)));
        assert_eq!(eval_code("add(1, 2, 3, 4)"), Ok(DynamicValue::Integer(10)));
        assert_eq!(eval_code("sub(1, 2)"), Ok(DynamicValue::Integer(-1)));
        assert_eq!(eval_code("mul(1, 2)"), Ok(DynamicValue::Integer(2)));
        assert_eq!(eval_code("mul(3, 1.5)"), Ok(DynamicValue::Float(4.5)));
        assert_eq!(eval_code("div(3, 2)"), Ok(DynamicValue::Float(1.5)));
        assert_eq!(eval_code("idiv(4.5, 2)"), Ok(DynamicValue::Integer(2)));
        assert_eq!(eval_code("idiv(-4.5, 2)"), Ok(DynamicValue::Integer(-3)));
    }

    #[test]
    fn test_lower() {
        assert_eq!(eval_code("lower(surname)"), Ok(DynamicValue::from("smith")));
    }

    #[test]
    fn test_upper() {
        assert_eq!(eval_code("upper(name)"), Ok(DynamicValue::from("JOHN")));
    }

    #[test]
    fn test_count() {
        assert_eq!(eval_code("count(name, 'h')"), Ok(DynamicValue::Integer(1)));
    }

    #[test]
    fn test_concat() {
        assert_eq!(
            eval_code("concat(name, ' ', lower(surname))"),
            Ok(DynamicValue::from("john smith"))
        );
    }

    #[test]
    fn test_coalesce() {
        assert_eq!(
            eval_code("coalesce(null, false, 'test')"),
            Ok(DynamicValue::from("test"))
        );
    }

    #[test]
    fn test_bool() {
        assert_eq!(eval_code("not(true)"), Ok(DynamicValue::from(false)));
        assert_eq!(eval_code("and(true, false)"), Ok(DynamicValue::from(false)));
        assert_eq!(
            eval_code("and(true, true, false)"),
            Ok(DynamicValue::from(false))
        );
        assert_eq!(eval_code("or(true, false)"), Ok(DynamicValue::from(true)));
        assert_eq!(
            eval_code("or(false, false, true)"),
            Ok(DynamicValue::from(true))
        );
    }

    #[test]
    fn test_pathjoin() {
        assert_eq!(
            eval_code("pathjoin('one', 'two', 'three')"),
            Ok(DynamicValue::from("one/two/three"))
        );
    }

    #[test]
    fn test_first() {
        assert_eq!(eval_code("first(name)"), Ok(DynamicValue::from("j")));
        assert_eq!(
            eval_code("first(split(name, 'h', 1))"),
            Ok(DynamicValue::from("jo"))
        );
    }

    #[test]
    fn test_last() {
        assert_eq!(eval_code("last(name)"), Ok(DynamicValue::from("n")));
        assert_eq!(
            eval_code("last(split(name, 'o', 1))"),
            Ok(DynamicValue::from("hn"))
        );
    }

    #[test]
    fn test_get() {
        assert_eq!(eval_code("get('test', 0)"), Ok(DynamicValue::from("t")));
        assert_eq!(eval_code("get('test', 7, 4)"), Ok(DynamicValue::from(4)));
        assert_eq!(eval_code("'test'[1]"), Ok(DynamicValue::from("e")));

        assert_eq!(
            eval_code("get({'one': {'two': [1, 2, 3]}}, ['one', 'two', 1])"),
            Ok(DynamicValue::from(2))
        );
    }

    #[test]
    fn test_slice() {
        assert_eq!(
            eval_code("slice('abcde', 2)"),
            Ok(DynamicValue::from("cde"))
        );
        assert_eq!(eval_code("'abcde'[2:]"), Ok(DynamicValue::from("cde")));
        assert_eq!(
            eval_code("slice('abcde', -2)"),
            Ok(DynamicValue::from("de"))
        );
        assert_eq!(
            eval_code("slice('abcde', -1, 3)"),
            Ok(DynamicValue::from(""))
        );
        assert_eq!(
            eval_code("slice('abcde', -1, -3)"),
            Ok(DynamicValue::from(""))
        );
        assert_eq!(
            eval_code("slice('abcde', 1, 3)"),
            Ok(DynamicValue::from("bc"))
        );
        assert_eq!(eval_code("'abcde'[1:3]"), Ok(DynamicValue::from("bc")));
        assert_eq!(
            eval_code("slice('abcde', 1, -2)"),
            Ok(DynamicValue::from("bc"))
        );
        assert_eq!(eval_code("slice('abcde', 5)"), Ok(DynamicValue::from("")));
        assert_eq!(eval_code("slice('abcde', 10)"), Ok(DynamicValue::from("")));
        assert_eq!(
            eval_code("slice('abcde', -10)"),
            Ok(DynamicValue::from("abcde"))
        );
        assert_eq!(
            eval_code("slice('abcde', 10, -20)"),
            Ok(DynamicValue::from(""))
        );
    }

    #[test]
    fn test_trim() {
        assert_eq!(eval_code("trim(' test ')"), Ok(DynamicValue::from("test")));
        assert_eq!(
            eval_code("ltrim(' test ')"),
            Ok(DynamicValue::from("test "))
        );
        assert_eq!(
            eval_code("rtrim(' test ')"),
            Ok(DynamicValue::from(" test"))
        );

        assert_eq!(eval_code("trim('test', 't')"), Ok(DynamicValue::from("es")));
        assert_eq!(
            eval_code("ltrim('test', 't')"),
            Ok(DynamicValue::from("est"))
        );
        assert_eq!(
            eval_code("rtrim('test', 't')"),
            Ok(DynamicValue::from("tes"))
        );
    }

    #[test]
    fn test_abs() {
        assert_eq!(eval_code("abs(-5)"), Ok(DynamicValue::Integer(5)));
        assert_eq!(eval_code("abs(-5.0)"), Ok(DynamicValue::Float(5.0)));
    }

    #[test]
    fn test_contains() {
        assert_eq!(
            eval_code("contains('hello', 'ell')"),
            Ok(DynamicValue::from(true))
        );
        assert_eq!(
            eval_code("contains('hello', /l{2}/)"),
            Ok(DynamicValue::from(true))
        );
        assert_eq!(
            eval_code("contains('hello', /l{3}/)"),
            Ok(DynamicValue::from(false))
        );
        assert_eq!(
            eval_code("contains('hello', /L{2}/i)"),
            Ok(DynamicValue::from(true))
        );
    }

    #[test]
    fn test_replace() {
        assert_eq!(
            eval_code("replace('hello', 'l', 't')"),
            Ok(DynamicValue::from("hetto"))
        );
        assert_eq!(
            eval_code("replace('hello', /l+O/i, 't')"),
            Ok(DynamicValue::from("het"))
        );
        assert_eq!(
            eval_code("replace('hello', /(he)llo/i, '$1')"),
            Ok(DynamicValue::from("he"))
        );
    }

    #[test]
    fn test_escape_regex() {
        assert_eq!(
            eval_code("escape_regex('(hello)')"),
            Ok(DynamicValue::from(r"\(hello\)"))
        );
        assert_eq!(
            eval_code("escape_regex('Hey. How are doing ?')"),
            Ok(DynamicValue::from(r"Hey\. How are doing \?"))
        );
    }

    #[test]
    fn test_if() {
        assert_eq!(eval_code("if(true, 3, 2)"), Ok(DynamicValue::from(3)));
        assert_eq!(
            eval_code("if(eq(2, 2), add(1, 3), sub(1, 0))"),
            Ok(DynamicValue::from(4))
        );
        assert_eq!(
            eval_code("if(if(if(true, true), true), if(false, add(1, 2), add(4, 5)))"),
            Ok(DynamicValue::from(9))
        );
    }

    #[test]
    fn test_unless() {
        assert_eq!(eval_code("unless(true, 3, 2)"), Ok(DynamicValue::from(2)));
    }

    #[test]
    fn test_neg() {
        assert_eq!(eval_code("neg(-1)"), Ok(DynamicValue::from(1)));
        assert_eq!(eval_code("neg(1)"), Ok(DynamicValue::from(-1)));
        assert_eq!(eval_code("neg(1.5)"), Ok(DynamicValue::from(-1.5)));
        assert_eq!(eval_code("neg(0)"), Ok(DynamicValue::from(0)));
        assert_eq!(eval_code("neg(0.0)"), Ok(DynamicValue::from(0.0)));
    }

    #[test]
    fn test_compact() {
        assert_eq!(
            eval_code("compact(split('', '|'))"),
            Ok(DynamicValue::from(vec![]))
        );
    }

    #[test]
    fn test_col() {
        assert_eq!(eval_code("col('name')"), Ok(DynamicValue::from("john")));
        assert_eq!(eval_code("col(1)"), Ok(DynamicValue::from("SMITH")));
        assert_eq!(eval_code("col(1.0)"), Ok(DynamicValue::from("SMITH")));
        assert_eq!(
            eval_code("col('surname', 0)"),
            Ok(DynamicValue::from("SMITH"))
        );
        assert_eq!(
            eval_code("col('surname', 1)"),
            Err(RunError::Prepare(ConcretizationError::ColumnNotFound(
                ColumIndexationBy::NameAndNth(("surname".to_string(), 1))
            )))
        );
        assert_eq!(
            eval_code("col(concat('sur', 'name'))"),
            Ok(DynamicValue::from("SMITH"))
        );
        assert_eq!(
            eval_code("col(concat('sur', 'name'), 1 - 1)"),
            Ok(DynamicValue::from("SMITH"))
        );
    }

    #[test]
    fn test_fmt() {
        assert_eq!(
            eval_code("fmt('Hello {}', 'John')"),
            Ok(DynamicValue::from("Hello John"))
        );
        assert_eq!(
            eval_code("fmt('Hello {} {}', 'John', 45)"),
            Ok(DynamicValue::from("Hello John 45"))
        );
    }

    #[test]
    fn test_ceil_floor_round() {
        assert_eq!(eval_code("ceil(2.3)"), Ok(DynamicValue::from(3)));
        assert_eq!(eval_code("ceil(4.8)"), Ok(DynamicValue::from(5)));
        assert_eq!(eval_code("floor(2.3)"), Ok(DynamicValue::from(2)));
        assert_eq!(eval_code("floor(-3.6)"), Ok(DynamicValue::from(-4)));
        assert_eq!(eval_code("round(2.3)"), Ok(DynamicValue::from(2)));
        assert_eq!(eval_code("round(3)"), Ok(DynamicValue::from(3)));
    }

    #[test]
    fn test_log_sqrt() {
        assert_eq!(eval_code("log(1)"), Ok(DynamicValue::from(0.0)));
        assert_eq!(
            eval_code("log(3.5)"),
            Ok(DynamicValue::from(1.252762968495368))
        );
        assert_eq!(eval_code("sqrt(4)"), Ok(DynamicValue::from(2.0)));
        assert_eq!(eval_code("sqrt(100)"), Ok(DynamicValue::from(10.0)));
    }

    #[test]
    fn test_md5() {
        assert_eq!(
            eval_code("md5('test')"),
            Ok(DynamicValue::from("098f6bcd4621d373cade4e832627b4f6"))
        );
    }

    #[test]
    fn test_pow() {
        assert_eq!(eval_code("pow(2, 4)"), Ok(DynamicValue::from(16)));
    }

    #[test]
    fn test_mod() {
        assert_eq!(eval_code("mod(8, 2)"), Ok(DynamicValue::from(0)));
    }

    #[test]
    fn test_infix_operators() {
        assert_eq!(eval_code("1 + 2"), Ok(DynamicValue::from(3)));
        assert_eq!(eval_code("1 - 2"), Ok(DynamicValue::from(-1)));
        assert_eq!(eval_code("2 * 2"), Ok(DynamicValue::from(4)));
        assert_eq!(eval_code("1 / 2"), Ok(DynamicValue::from(0.5)));
        assert_eq!(eval_code("1 // 2"), Ok(DynamicValue::from(0)));
        assert_eq!(eval_code("2 ** 4"), Ok(DynamicValue::from(16)));
        assert_eq!(eval_code("8 % 2"), Ok(DynamicValue::from(0)));

        assert_eq!(eval_code("'he'.'llo'"), Ok(DynamicValue::from("hello")));

        assert_eq!(eval_code("true && false"), Ok(DynamicValue::from(false)));
        assert_eq!(eval_code("true and false"), Ok(DynamicValue::from(false)));
        assert_eq!(eval_code("true || false"), Ok(DynamicValue::from(true)));
        assert_eq!(eval_code("true or false"), Ok(DynamicValue::from(true)));

        assert_eq!(
            eval_code("true && (true && (false || true) || false && false) && false"),
            Ok(DynamicValue::from(false))
        );

        assert_eq!(eval_code("'h' in 'hello'"), Ok(DynamicValue::from(true)));
        assert_eq!(
            eval_code("'h' not in 'hello'"),
            Ok(DynamicValue::from(false))
        );
        assert_eq!(eval_code("3 in [1, 2, 3]"), Ok(DynamicValue::from(true)));
        assert_eq!(
            eval_code("'3' in ['1', '2', '3']"),
            Ok(DynamicValue::from(true))
        );
        assert_eq!(
            eval_code("3 in ['1', '2', '3']"),
            Ok(DynamicValue::from(true))
        );
        assert_eq!(eval_code("'3' in [1, 2, 3]"), Ok(DynamicValue::from(true)));
        assert_eq!(eval_code("4 in [1, 2, 3]"), Ok(DynamicValue::from(false)));
        assert_eq!(eval_code("'4' in [1, 2, 3]"), Ok(DynamicValue::from(false)));

        assert_eq!(eval_code("!true"), Ok(DynamicValue::from(false)));
        assert_eq!(eval_code("!!!true"), Ok(DynamicValue::from(false)));

        assert_eq!(eval_code("1 == 2"), Ok(DynamicValue::from(false)));
        assert_eq!(eval_code("1 > 2"), Ok(DynamicValue::from(false)));
        assert_eq!(eval_code("1 >= 2"), Ok(DynamicValue::from(false)));
        assert_eq!(eval_code("1 < 2"), Ok(DynamicValue::from(true)));
        assert_eq!(eval_code("1 <= 2"), Ok(DynamicValue::from(true)));
        assert_eq!(eval_code("1 != 2"), Ok(DynamicValue::from(true)));

        assert_eq!(eval_code("'a' eq 'b'"), Ok(DynamicValue::from(false)));
        assert_eq!(eval_code("'a' gt 'b'"), Ok(DynamicValue::from(false)));
        assert_eq!(eval_code("'a' ge 'b'"), Ok(DynamicValue::from(false)));
        assert_eq!(eval_code("'a' lt 'b'"), Ok(DynamicValue::from(true)));
        assert_eq!(eval_code("'a' le 'b'"), Ok(DynamicValue::from(true)));
        assert_eq!(eval_code("'a' ne 'b'"), Ok(DynamicValue::from(true)));
    }

    #[test]
    fn test_bytesize() {
        assert_eq!(
            eval_code("bytesize(2510)"),
            Ok(DynamicValue::from("2.5 KB"))
        );
        assert_eq!(eval_code("bytesize(0)"), Ok(DynamicValue::from("0 B")));
    }

    #[test]
    fn test_map() {
        assert_eq!(
            eval_code("{hello: 'world'} | get(_, 'hello')"),
            Ok(DynamicValue::from("world"))
        );

        assert_eq!(
            eval_code("{hello: name} | get(_, 'hello')"),
            Ok(DynamicValue::from("john"))
        );
    }

    #[test]
    fn test_json() {
        assert_eq!(
            eval_code("json_parse('[1, 2, 3]') | get(_, 1)"),
            Ok(DynamicValue::from(2))
        );

        assert_eq!(
            eval_code("json_parse('{\"one\": 34}') | get(_, 'one')"),
            Ok(DynamicValue::from(34))
        );
    }

    #[test]
    fn test_minmax() {
        assert_eq!(eval_code("min(1, 2, -5, 4)"), Ok(DynamicValue::from(-5)));
        assert_eq!(eval_code("max(1, 2, -5, 4)"), Ok(DynamicValue::from(4)));

        assert_eq!(
            eval_code("argmin([1, 2, -5, 4])"),
            Ok(DynamicValue::from(2))
        );
        assert_eq!(
            eval_code("argmin([1, 2, -5, 4], ['a', 'b', 'c', 'd'])"),
            Ok(DynamicValue::from("c"))
        );
        assert_eq!(
            eval_code("argmin([1, 2, -5, 4], ['a'])"),
            Ok(DynamicValue::None)
        );
        assert_eq!(eval_code("argmin([a, b])"), Ok(DynamicValue::from(0)));

        assert_eq!(
            eval_code("argmax([1, 2, -5, 4])"),
            Ok(DynamicValue::from(3))
        );
        assert_eq!(
            eval_code("argmax([1, 2, -5, 4], ['a', 'b', 'c', 'd'])"),
            Ok(DynamicValue::from("d"))
        );
        assert_eq!(
            eval_code("argmax([1, 2, -5, 4], ['a'])"),
            Ok(DynamicValue::None)
        );
        assert_eq!(eval_code("argmax([a, b])"), Ok(DynamicValue::from(1)));
    }

    #[test]
    fn test_timestamp() {
        let tz = TimeZone::system();
        let timestamp = Timestamp::from_second(1645805387).unwrap();
        let zoned = timestamp.to_zoned(tz);

        assert_eq!(
            eval_code("timestamp(1645805387)"),
            Ok(DynamicValue::from(zoned))
        )
    }
}
