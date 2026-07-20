mod common;

use common::must_err;
use factorio_codegen::{LuaGenerator, LuaGeneratorError};
use factorio_ir::{
    block::Block,
    function::Function,
    module::{Module, Symbol},
    scope::Scope,
    stage::Stage,
    statement::Statement,
};

#[test]
fn rejects_exported_local_functions() {
    let module = Module {
        name: "broken".to_string(),
        stage: Stage::Control,
        body: Block { statements: vec![] },
        imports: vec![],
        submodules: vec![],
        locales: vec![],
        pending_locales: vec![],
        vtables: vec![],
        symbols: vec![Symbol {
            scope: Scope::Private,
            statement: Statement::FunctionDecl(Function {
                name: "secret".to_string(),
                params: vec![],
                body: Block { statements: vec![] },
                doc: None,
                debug: None,
                event: None,
                event_filter: None,
                export: None,
            }),
        }],
    };

    let error = must_err(LuaGenerator::new().generate_module(&module));
    assert_eq!(
        error,
        LuaGeneratorError::FunctionLocalAndExported("secret".to_string())
    );
}
