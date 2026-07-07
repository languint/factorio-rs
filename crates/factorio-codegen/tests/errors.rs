use factorio_codegen::{LuaGenerator, LuaGeneratorError};
use factorio_ir::{
    block::Block,
    function::Function,
    module::{Module, Symbol},
    scope::Scope,
    statement::Statement,
};

#[test]
fn rejects_exported_local_functions() {
    let module = Module {
        name: "broken".to_string(),
        body: Block { statements: vec![] },
        imports: vec![],
        submodules: vec![],
        symbols: vec![Symbol {
            scope: Scope::Private,
            statement: Statement::FunctionDecl(Function {
                name: "secret".to_string(),
                params: vec![],
                body: Block { statements: vec![] },
            }),
        }],
    };

    let error = LuaGenerator::new().generate_module(&module).unwrap_err();
    assert_eq!(
        error,
        LuaGeneratorError::FunctionLocalAndExported("secret".to_string())
    );
}
