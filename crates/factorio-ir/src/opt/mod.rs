mod concat;
mod hoist;
mod inline;
mod simplify;

use crate::module::Module;

/// Run all IR optimization passes on every module.
pub fn optimize_modules(modules: &mut [Module]) {
    for module in modules {
        optimize_module(module);
    }
}

fn optimize_module(module: &mut Module) {
    hoist::optimize_module(module);
    simplify::optimize_module(module);
    inline::optimize_module(module);
    // Inlining can expose shapes simplify already knows (bool if->expr, etc.).
    simplify::optimize_module(module);
    concat::optimize_module(module);
}

#[cfg(test)]
mod tests;
