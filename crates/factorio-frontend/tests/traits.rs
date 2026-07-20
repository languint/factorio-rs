#![allow(
    clippy::expect_used,
    clippy::literal_string_with_formatting_args,
    clippy::needless_raw_string_hashes,
    clippy::panic,
    clippy::unwrap_used
)]
mod common;

use common::must_ok_parse;
use factorio_frontend::parse_module;
use factorio_ir::{expression::Expression, statement::Statement};

#[test]
fn trait_impl_and_method_call_parses() {
    let source = r#"
struct Point {
    x: i32,
}

trait Display {
    fn show(&self) -> i32;
}

impl Display for Point {
    fn show(&self) -> i32 {
        self.x
    }
}

pub fn run(p: Point) -> i32 {
    p.show()
}
"#;
    let module = must_ok_parse(parse_module(source, "shared.traits"));

    let Statement::StructDecl(struct_decl) = &module.body.statements[0] else {
        panic!(
            "expected private struct, got {:?}",
            module.body.statements[0]
        );
    };
    assert_eq!(struct_decl.name, "Point");
    assert_eq!(struct_decl.methods.len(), 1);
    assert_eq!(struct_decl.methods[0].name, "show");

    assert_eq!(module.vtables.len(), 1);
    assert_eq!(module.vtables[0].name, "__vt_Display_Point");
    assert_eq!(module.vtables[0].methods, vec!["show".to_string()]);

    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::Return(Some(Expression::Call { func, args })) = &function.body.statements[0]
    else {
        panic!(
            "expected Type.method call, got {:?}",
            function.body.statements[0]
        );
    };
    assert_eq!(
        func.as_ref(),
        &Expression::QualifiedPath {
            segments: vec!["Point".to_string(), "show".to_string()],
        }
    );
    assert_eq!(args.len(), 1);
}

#[test]
fn trait_default_method_filled_into_impl() {
    let source = r#"
struct Point {
    x: i32,
}

trait Display {
    fn show(&self) -> i32 {
        self.x
    }
}

impl Display for Point {}

pub fn run(p: Point) -> i32 {
    p.show()
}
"#;
    let module = must_ok_parse(parse_module(source, "shared.traits"));
    let Statement::StructDecl(struct_decl) = &module.body.statements[0] else {
        panic!("expected struct");
    };
    assert_eq!(struct_decl.methods.len(), 1);
    assert_eq!(struct_decl.methods[0].name, "show");
}

#[test]
fn trait_with_associated_type_and_self_path() {
    let source = r#"
trait Mapper {
    type Output;
    fn map_value(&self) -> Self::Output;
}

struct N {
    n: i32,
}

impl Mapper for N {
    type Output = i32;
    fn map_value(&self) -> Self::Output {
        self.n
    }
}

pub fn run(x: N) -> i32 {
    x.map_value()
}
"#;
    let module = must_ok_parse(parse_module(source, "shared.traits"));
    let Statement::StructDecl(struct_decl) = &module.body.statements[0] else {
        panic!("expected struct");
    };
    assert_eq!(struct_decl.methods.len(), 1);
    assert_eq!(struct_decl.methods[0].name, "map_value");
    assert_eq!(
        struct_decl.methods[0]
            .debug
            .as_ref()
            .map(|d| d.return_type.as_deref()),
        Some(Some("i32"))
    );
}

#[test]
fn dyn_cast_with_associated_type_rejected() {
    let source = r#"
trait Mapper {
    type Output;
    fn map_value(&self) -> Self::Output;
}
struct N { n: i32 }
impl Mapper for N {
    type Output = i32;
    fn map_value(&self) -> Self::Output { self.n }
}
pub fn run(x: N) -> i32 {
    let d = &x as &dyn Mapper;
    d.map_value()
}
"#;
    let err = parse_module(source, "shared.traits").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("object-safe") || msg.contains("associated type"),
        "unexpected error: {msg}"
    );
}

#[test]
fn missing_associated_type_in_impl_rejected() {
    let source = r#"
trait Mapper {
    type Output;
    fn map_value(&self) -> Self::Output;
}
struct N { n: i32 }
impl Mapper for N {
    fn map_value(&self) -> Self::Output { self.n }
}
"#;
    let err = parse_module(source, "shared.traits").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("missing associated type") || msg.contains("Output"),
        "unexpected error: {msg}"
    );
}

#[test]
fn trait_with_generics_rejected() {
    let source = r#"
trait Display<T> {
    fn show(&self) -> T;
}
"#;
    let err = parse_module(source, "shared.traits").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("generics") || msg.contains("trait `Display`"),
        "unexpected error: {msg}"
    );
}

#[test]
fn trait_method_name_clash_with_inherent_rejected() {
    let source = r#"
struct Point {
    x: i32,
}

impl Point {
    fn show(&self) -> i32 {
        self.x
    }
}

trait Display {
    fn show(&self) -> i32;
}

impl Display for Point {
    fn show(&self) -> i32 {
        self.x
    }
}
"#;
    let err = parse_module(source, "shared.traits").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("already defined") || msg.contains("show"),
        "unexpected error: {msg}"
    );
}

#[test]
fn dyn_cast_and_method_call_lower() {
    let source = r#"
struct Point {
    x: i32,
}

trait Display {
    fn show(&self) -> i32;
}

impl Display for Point {
    fn show(&self) -> i32 {
        self.x
    }
}

pub fn run() -> i32 {
    let p = Point { x: 1 };
    let d = p as &dyn Display;
    d.show()
}
"#;
    let module = must_ok_parse(parse_module(source, "shared.traits"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };

    let mut saw_fat = false;
    let mut saw_dyn_call = false;
    for stmt in &function.body.statements {
        match stmt {
            Statement::VariableDecl {
                value: Expression::FatPointer { vtable, .. },
                ..
            } => {
                assert_eq!(vtable, "__vt_Display_Point");
                saw_fat = true;
            }
            Statement::Return(Some(Expression::DynMethodCall { method, .. })) => {
                assert_eq!(method, "show");
                saw_dyn_call = true;
            }
            _ => {}
        }
    }
    assert!(saw_fat, "expected FatPointer in body: {:?}", function.body);
    assert!(saw_dyn_call, "expected DynMethodCall: {:?}", function.body);
}

#[test]
fn dyn_cast_borrow_of_local() {
    let source = r#"
struct Point { x: i32 }
trait Display { fn show(&self) -> i32; }
impl Display for Point { fn show(&self) -> i32 { self.x } }
pub fn run() -> i32 {
    let p = Point { x: 1 };
    let d = &p as &dyn Display;
    d.show()
}
"#;
    let module = must_ok_parse(parse_module(source, "shared.traits"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let mut saw_fat = false;
    for stmt in &function.body.statements {
        if let Statement::VariableDecl {
            value: Expression::FatPointer { vtable, .. },
            ..
        } = stmt
        {
            assert_eq!(vtable, "__vt_Display_Point");
            saw_fat = true;
        }
    }
    assert!(
        saw_fat,
        "expected FatPointer for `&p as &dyn Display`: {:?}",
        function.body
    );
}

#[test]
fn dyn_cast_as_call_arg_without_ref() {
    let source = r#"
struct Point { x: i32 }
trait Display { fn show(&self) -> i32; }
impl Display for Point { fn show(&self) -> i32 { self.x } }
fn priority(d: &dyn Display) -> i32 { d.show() }
pub fn run(p: Point) -> i32 {
    priority(p as &dyn Display)
}
"#;
    let module = must_ok_parse(parse_module(source, "shared.traits"));
    let run = module
        .symbols
        .iter()
        .find_map(|sym| match &sym.statement {
            Statement::FunctionDecl(f) if f.name == "run" => Some(f),
            _ => None,
        })
        .or_else(|| {
            module.body.statements.iter().find_map(|s| match s {
                Statement::FunctionDecl(f) if f.name == "run" => Some(f),
                _ => None,
            })
        })
        .expect("expected run function");

    let Statement::Return(Some(Expression::Call { args, .. })) = &run.body.statements[0] else {
        panic!("expected return call, got {:?}", run.body.statements[0]);
    };
    assert!(
        matches!(args.first(), Some(Expression::FatPointer { vtable, .. }) if vtable == "__vt_Display_Point"),
        "expected FatPointer arg, got {args:?}"
    );
}

#[test]
fn dyn_call_arg_auto_coerces_without_cast() {
    let source = r#"
struct Point { x: i32 }
trait Display { fn show(&self) -> i32; }
impl Display for Point { fn show(&self) -> i32 { self.x } }
fn priority(d: &dyn Display) -> i32 { d.show() }
pub fn run() -> i32 {
    priority(&Point { x: 7 })
}
"#;
    let module = must_ok_parse(parse_module(source, "shared.traits"));
    let run = module
        .symbols
        .iter()
        .find_map(|sym| match &sym.statement {
            Statement::FunctionDecl(f) if f.name == "run" => Some(f),
            _ => None,
        })
        .or_else(|| {
            module.body.statements.iter().find_map(|s| match s {
                Statement::FunctionDecl(f) if f.name == "run" => Some(f),
                _ => None,
            })
        })
        .expect("expected run function");

    let Statement::Return(Some(Expression::Call { args, .. })) = &run.body.statements[0] else {
        panic!("expected return call, got {:?}", run.body.statements[0]);
    };
    assert!(
        matches!(
            args.first(),
            Some(Expression::FatPointer { vtable, data })
                if vtable == "__vt_Display_Point"
                    && matches!(
                        data.as_ref(),
                        Expression::StructLiteral { struct_name, .. }
                            if struct_name.as_deref() == Some("Point")
                    )
        ),
        "expected auto FatPointer arg, got {args:?}"
    );
}

#[test]
fn dyn_call_arg_auto_coerces_borrowed_local() {
    let source = r#"
struct Point { x: i32 }
trait Display { fn show(&self) -> i32; }
impl Display for Point { fn show(&self) -> i32 { self.x } }
fn priority(d: &dyn Display) -> i32 { d.show() }
pub fn run() -> i32 {
    let p = Point { x: 3 };
    priority(&p)
}
"#;
    let module = must_ok_parse(parse_module(source, "shared.traits"));
    let run = module
        .symbols
        .iter()
        .find_map(|sym| match &sym.statement {
            Statement::FunctionDecl(f) if f.name == "run" => Some(f),
            _ => None,
        })
        .or_else(|| {
            module.body.statements.iter().find_map(|s| match s {
                Statement::FunctionDecl(f) if f.name == "run" => Some(f),
                _ => None,
            })
        })
        .expect("expected run function");

    let Statement::Return(Some(Expression::Call { args, .. })) =
        &run.body.statements.last().unwrap()
    else {
        panic!("expected return call, got {:?}", run.body.statements);
    };
    assert!(
        matches!(
            args.first(),
            Some(Expression::FatPointer { vtable, data })
                if vtable == "__vt_Display_Point"
                    && matches!(data.as_ref(), Expression::Identifier(name) if name == "p")
        ),
        "expected auto FatPointer for `&p`, got {args:?}"
    );
}

#[test]
fn struct_literal_method_call_uses_type_table() {
    let source = r#"
struct Point { x: i32 }
trait Display { fn show(&self) -> i32; }
impl Display for Point { fn show(&self) -> i32 { self.x } }
pub fn run() -> i32 {
    Point { x: 1 }.show()
}
"#;
    let module = must_ok_parse(parse_module(source, "shared.traits"));
    let Statement::FunctionDecl(function) = &module.symbols[0].statement else {
        panic!("expected function");
    };
    let Statement::Return(Some(Expression::Call { func, args })) = &function.body.statements[0]
    else {
        panic!(
            "expected Point.show(...) call, got {:?}",
            function.body.statements[0]
        );
    };
    assert_eq!(
        func.as_ref(),
        &Expression::QualifiedPath {
            segments: vec!["Point".to_string(), "show".to_string()],
        }
    );
    assert_eq!(args.len(), 1);
    assert!(matches!(
        &args[0],
        Expression::StructLiteral {
            struct_name: Some(name),
            ..
        } if name == "Point"
    ));
}
