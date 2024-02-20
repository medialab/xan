use fmt;
use std::borrow::Cow;

use csv::ByteRecord;
use regex::RegexBuilder;

use super::error::{
    BindingError, CallError, ConcretizationError, EvaluationError, SpecifiedBindingError,
    SpecifiedCallError,
};
use super::functions::{get_function, Function};
use super::parser::{parse_and_optimize_pipeline, Expr, FunctionCall, Pipeline};
use super::types::{
    BoundArgument, BoundArguments, ColumIndexationBy, DynamicValue, EvaluationResult, HeadersIndex,
    Variables,
};

#[derive(Debug, Clone)]
pub struct EvaluationContext<'a> {
    pub headers_index: &'a HeadersIndex,
    pub record: &'a ByteRecord,
    pub last_value: Option<&'a DynamicValue>,
    pub variables: &'a Variables<'a>,
}

#[derive(Debug, Clone)]
pub enum ConcreteArgument {
    Variable(String),
    Column(usize),
    Str(DynamicValue),
    Float(DynamicValue),
    Int(DynamicValue),
    Bool(DynamicValue),
    Regex(DynamicValue),
    Call(ConcreteFunctionCall),
    Null,
    Underscore,
}

impl ConcreteArgument {
    fn bind<'a>(
        &'a self,
        context: &'a EvaluationContext,
    ) -> Result<BoundArgument<'a>, BindingError> {
        Ok(match self {
            Self::Str(value) => Cow::Borrowed(value),
            Self::Float(value) => Cow::Borrowed(value),
            Self::Int(value) => Cow::Borrowed(value),
            Self::Bool(value) => Cow::Borrowed(value),
            Self::Underscore => {
                Cow::Borrowed(context.last_value.unwrap_or_else(|| &DynamicValue::None))
            }
            Self::Null => Cow::Owned(DynamicValue::None),
            Self::Column(index) => match context.record.get(*index) {
                None => return Err(BindingError::ColumnOutOfRange(*index)),
                Some(cell) => match std::str::from_utf8(cell) {
                    Err(_) => return Err(BindingError::UnicodeDecodeError),
                    Ok(value) => Cow::Owned(DynamicValue::from(value)),
                },
            },
            Self::Variable(name) => match context.variables.get::<str>(name) {
                Some(value) => Cow::Borrowed(value),
                None => return Err(BindingError::UnknownVariable(name.clone())),
            },
            Self::Regex(value) => Cow::Borrowed(value),
            Self::Call(_) => return Err(BindingError::IllegalBinding),
        })
    }

    fn evaluate<'a>(&'a self, context: &'a EvaluationContext) -> EvaluationResult<'a> {
        match self {
            Self::Call(function_call) => function_call.run(context),
            _ => self.bind(context).map_err(|err| {
                EvaluationError::Binding(SpecifiedBindingError {
                    function_name: "<expr>".to_string(),
                    arg_index: None,
                    reason: err,
                })
            }),
        }
    }

    fn is_index_variable(&self) -> bool {
        match self {
            Self::Variable(name) => name == "index",
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct ConcreteSubroutine {
    name: String,
    function: Function,
    args: Vec<ConcreteArgument>,
}

impl ConcreteSubroutine {
    fn run<'a>(&'a self, context: &'a EvaluationContext) -> EvaluationResult<'a> {
        let mut bound_args = BoundArguments::new();

        for (i, arg) in self.args.iter().enumerate() {
            match arg {
                ConcreteArgument::Call(sub_function_call) => {
                    bound_args.push(sub_function_call.run(context)?);
                }
                _ => bound_args.push(arg.bind(context).map_err(|err| {
                    EvaluationError::Binding(SpecifiedBindingError {
                        function_name: self.name.to_string(),
                        arg_index: Some(i),
                        reason: err,
                    })
                })?),
            }
        }

        match (self.function)(bound_args) {
            Ok(value) => Ok(Cow::Owned(value)),
            Err(err) => Err(EvaluationError::Call(SpecifiedCallError {
                function_name: self.name.clone(),
                reason: err,
            })),
        }
    }
}

// NOTE: in older rust versions, Debug cannot be derived
// correctly from `fn` and it will not compile without
// this custom `Debug` implementation
impl fmt::Debug for ConcreteSubroutine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConcreteSubroutine")
            .field("name", &self.name)
            .field("function", &"<function>")
            .field("args", &self.args)
            .finish()
    }
}

#[derive(Debug, Clone)]
enum StatementKind {
    If(bool),
    Col,
}

impl StatementKind {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "if" => Some(Self::If(false)),
            "unless" => Some(Self::If(true)),
            "col" => Some(Self::Col),
            _ => None,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::If(reverse) => {
                if *reverse {
                    "unless"
                } else {
                    "if"
                }
            }
            Self::Col => "col",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConcreteStatement {
    kind: StatementKind,
    args: Vec<ConcreteArgument>,
}

impl ConcreteStatement {
    fn run<'a>(&'a self, context: &'a EvaluationContext) -> EvaluationResult<'a> {
        match self.kind {
            StatementKind::If(reverse) => {
                let arity = self.args.len();

                if !(2..=3).contains(&arity) {
                    return Err(EvaluationError::Call(SpecifiedCallError {
                        function_name: self.kind.name().to_string(),
                        reason: CallError::from_invalid_range_arity(2..=3, arity),
                    }));
                }

                let condition = &self.args[0];
                let result = condition.evaluate(context)?;

                let mut branch: Option<&ConcreteArgument> = None;

                let mut go_left = result.is_truthy();

                if reverse {
                    go_left = !go_left;
                }

                if go_left {
                    branch = Some(&self.args[1]);
                } else if arity == 3 {
                    branch = Some(&self.args[2]);
                }

                match branch {
                    None => Ok(Cow::Owned(DynamicValue::None)),
                    Some(arg) => arg.evaluate(context),
                }
            }
            StatementKind::Col => {
                let arity = self.args.len();

                if !(1..=2).contains(&arity) {
                    return Err(EvaluationError::Call(SpecifiedCallError {
                        function_name: self.kind.name().to_string(),
                        reason: CallError::from_invalid_range_arity(1..=2, arity),
                    }));
                }

                let name_or_pos = self.args.first().unwrap().evaluate(context)?;
                let pos = match self.args.get(1) {
                    Some(p) => Some(p.evaluate(context)?),
                    None => None,
                };

                match ColumIndexationBy::from_bound_arguments(name_or_pos, pos) {
                    None => Err(EvaluationError::Call(SpecifiedCallError {
                        function_name: self.kind.name().to_string(),
                        reason: CallError::Custom("invalid arguments".to_string()),
                    })),
                    Some(indexation) => match context.headers_index.get(&indexation) {
                        None => Err(EvaluationError::Call(SpecifiedCallError {
                            function_name: self.kind.name().to_string(),
                            reason: CallError::ColumnNotFound(indexation),
                        })),
                        Some(index) => match std::str::from_utf8(&context.record[index]) {
                            Err(_) => Err(EvaluationError::Binding(SpecifiedBindingError {
                                function_name: self.kind.name().to_string(),
                                arg_index: None,
                                reason: BindingError::UnicodeDecodeError,
                            })),
                            Ok(value) => Ok(Cow::Owned(DynamicValue::from(value))),
                        },
                    },
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConcreteFunctionCall {
    Subroutine(ConcreteSubroutine),
    SpecialStatement(ConcreteStatement),
}

impl ConcreteFunctionCall {
    fn run<'a>(&'a self, context: &'a EvaluationContext) -> EvaluationResult<'a> {
        match self {
            Self::Subroutine(subroutine) => subroutine.run(context),
            Self::SpecialStatement(statement) => statement.run(context),
        }
    }

    fn has_index_variable(&self) -> bool {
        let args = match self {
            Self::SpecialStatement(statement) => &statement.args,
            Self::Subroutine(subroutine) => &subroutine.args,
        };

        args.iter().any(|arg| arg.is_index_variable())
    }
}

type ConcretePipeline = Vec<ConcreteArgument>;

fn concretize_call(
    call: FunctionCall,
    headers: &ByteRecord,
) -> Result<ConcreteArgument, ConcretizationError> {
    let function_name = call.name;
    let arity = call.args.len();

    if arity > 8 {
        return Err(ConcretizationError::TooManyArguments(arity));
    }

    // Statically analyzable col() function call
    if function_name == "col" {
        if !(1..=2).contains(&arity) {
            return Err(ConcretizationError::from_invalid_range_arity(
                "col".to_string(),
                1..=2,
                arity,
            ));
        }

        if let Some(column_indexation) = ColumIndexationBy::from_arguments(&call.args) {
            match column_indexation.find_column_index(headers) {
                Some(index) => return Ok(ConcreteArgument::Column(index)),
                None => return Err(ConcretizationError::ColumnNotFound(column_indexation)),
            };
        }
    }

    let mut concrete_args = Vec::new();

    for arg in call.args {
        concrete_args.push(concretize_expression(arg, headers)?);
    }

    Ok(if let Some(kind) = StatementKind::parse(&function_name) {
        ConcreteArgument::Call(ConcreteFunctionCall::SpecialStatement(ConcreteStatement {
            kind,
            args: concrete_args,
        }))
    } else {
        match get_function(&function_name) {
            None => return Err(ConcretizationError::UnknownFunction(function_name.clone())),
            Some(function_info) => {
                function_info
                    .1
                    .validate(&function_name, concrete_args.len())?;

                ConcreteArgument::Call(ConcreteFunctionCall::Subroutine(ConcreteSubroutine {
                    name: function_name.clone(),
                    function: function_info.0,
                    args: concrete_args,
                }))
            }
        }
    })
}

pub fn concretize_expression(
    expr: Expr,
    headers: &ByteRecord,
) -> Result<ConcreteArgument, ConcretizationError> {
    Ok(match expr {
        Expr::Underscore => ConcreteArgument::Underscore,
        Expr::Null => ConcreteArgument::Null,
        Expr::Bool(v) => ConcreteArgument::Bool(DynamicValue::Boolean(v)),
        Expr::Float(v) => ConcreteArgument::Float(DynamicValue::Float(v)),
        Expr::Int(v) => ConcreteArgument::Int(DynamicValue::Integer(v)),
        Expr::Str(v) => ConcreteArgument::Str(DynamicValue::String(v)),
        Expr::Identifier(name) => {
            let indexation = ColumIndexationBy::Name(name);

            match indexation.find_column_index(headers) {
                Some(index) => ConcreteArgument::Column(index),
                None => return Err(ConcretizationError::ColumnNotFound(indexation)),
            }
        }
        Expr::SpecialIdentifier(name) => ConcreteArgument::Variable(name),
        Expr::Regex(pattern, case_insensitive) => match RegexBuilder::new(&pattern)
            .case_insensitive(case_insensitive)
            .build()
        {
            Ok(regex) => ConcreteArgument::Regex(DynamicValue::Regex(regex)),
            Err(_) => return Err(ConcretizationError::InvalidRegex(pattern)),
        },
        Expr::Func(call) => concretize_call(call, headers)?,
    })
}

fn concretize_pipeline(
    pipeline: Pipeline,
    headers: &ByteRecord,
) -> Result<ConcretePipeline, ConcretizationError> {
    let mut concrete_pipeline: ConcretePipeline = Vec::new();

    for argument in pipeline {
        concrete_pipeline.push(concretize_expression(argument, headers)?);
    }

    Ok(concrete_pipeline)
}

pub fn eval_pipeline(
    pipeline: &ConcretePipeline,
    record: &ByteRecord,
    headers_index: &HeadersIndex,
    variables: &Variables,
) -> Result<DynamicValue, EvaluationError> {
    let mut last_value = DynamicValue::None;

    for arg in pipeline {
        last_value = arg
            .evaluate(&EvaluationContext {
                headers_index,
                record,
                variables,
                last_value: Some(&last_value),
            })?
            .into_owned();
    }

    Ok(last_value)
}

pub fn eval_expr(
    expr: &ConcreteArgument,
    record: &ByteRecord,
    headers_index: &HeadersIndex,
    variables: &Variables,
) -> Result<DynamicValue, EvaluationError> {
    let context = EvaluationContext {
        record,
        variables,
        headers_index,
        last_value: None,
    };

    expr.evaluate(&context).map(|value| value.into_owned())
}

#[derive(Clone)]
pub struct PipelineProgram<'a> {
    pipeline: ConcretePipeline,
    headers_index: HeadersIndex,
    variables: Variables<'a>,
    should_bind_index: bool,
}

impl<'a> PipelineProgram<'a> {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let pipeline = match parse_and_optimize_pipeline(code) {
            Err(_) => return Err(ConcretizationError::ParseError(code.to_string())),
            Ok(pipeline) => concretize_pipeline(pipeline, headers)?,
        };

        let should_bind_index = pipeline.iter().any(|arg| {
            if let ConcreteArgument::Call(call) = arg {
                call.has_index_variable()
            } else {
                arg.is_index_variable()
            }
        });

        Ok(Self {
            pipeline,
            variables: Variables::new(),
            headers_index: HeadersIndex::from_headers(headers),
            should_bind_index,
        })
    }

    pub fn run_with_record(&self, record: &ByteRecord) -> Result<DynamicValue, EvaluationError> {
        eval_pipeline(&self.pipeline, record, &self.headers_index, &self.variables)
    }

    pub fn set<'b>(&'b mut self, key: &'a str, value: DynamicValue) {
        if key == "index" && !self.should_bind_index {
            return;
        }

        self.variables.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::super::error::RunError;
    use super::*;

    type TestResult = Result<DynamicValue, RunError>;

    fn eval_code(code: &str) -> TestResult {
        let mut headers = ByteRecord::new();
        headers.push_field(b"name");
        headers.push_field(b"surname");
        headers.push_field(b"a");
        headers.push_field(b"b");

        let mut program = PipelineProgram::parse(code, &headers).map_err(RunError::Prepare)?;

        let mut record = ByteRecord::new();
        record.push_field(b"john");
        record.push_field(b"SMITH");
        record.push_field(b"34");
        record.push_field(b"62");

        program.set(&"index", DynamicValue::Integer(2));
        program
            .run_with_record(&record)
            .map_err(RunError::Evaluation)
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
    fn test_variable_binding() {
        assert_eq!(eval_code("add(%index, 2)"), Ok(DynamicValue::from(4)));
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
            Ok(DynamicValue::List(vec![
                DynamicValue::from("j"),
                DynamicValue::from("hn"),
            ]))
        );

        assert_eq!(
            eval_code("split(name, 'o', 1)"),
            Ok(DynamicValue::List(vec![
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
    fn test_slice() {
        assert_eq!(
            eval_code("slice('abcde', 2)"),
            Ok(DynamicValue::from("cde"))
        );
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
        assert_eq!(eval_code("fmt('test')"), Ok(DynamicValue::from("test")));
        assert_eq!(
            eval_code("fmt('Hello {}')"),
            Ok(DynamicValue::from("Hello {}"))
        );
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
}
