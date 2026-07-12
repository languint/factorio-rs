//! Standard Lua libraries available as Factorio globals (`math`, `string`, `table`).
//!
//! Stubs exist for `cargo check` / IDE support only. Calls lower to the real Lua
//! library methods. Overloads that need distinct Rust names (`random_int`,
//! `format_2`, ...) are remapped to the Lua name by the frontend.

/// Lua `math` library (Factorio uses deterministic implementations).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuaMath {
    pub pi: f64,
    pub huge: f64,
}

impl Default for LuaMath {
    fn default() -> Self {
        Self::new()
    }
}

impl LuaMath {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pi: 3.141_592_653_589_793,
            huge: f64::INFINITY,
        }
    }

    #[allow(unused_variables)]
    pub fn abs(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn acos(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn asin(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn atan(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn atan2(&self, y: f64, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn ceil(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn cos(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn cosh(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn deg(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn exp(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn floor(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn fmod(&self, x: f64, y: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn frexp(&self, x: f64) -> (f64, i32) {
        (0.0, 0)
    }
    #[allow(unused_variables)]
    pub fn ldexp(&self, x: f64, exp: i32) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn log(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn log10(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn max(&self, a: f64, b: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn min(&self, a: f64, b: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn modf(&self, x: f64) -> (f64, f64) {
        (0.0, 0.0)
    }
    #[allow(unused_variables)]
    pub fn pow(&self, x: f64, y: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn rad(&self, x: f64) -> f64 {
        0.0
    }
    /// `math.random()` - float in `[0, 1)`.
    pub fn random(&self) -> f64 {
        0.0
    }
    /// `math.random(n)` - integer in `[1, n]`. Lowers as `math.random`.
    #[allow(unused_variables)]
    pub fn random_int(&self, n: i64) -> i64 {
        0
    }
    /// `math.random(m, n)` - integer in `[m, n]`. Lowers as `math.random`.
    #[allow(unused_variables)]
    pub fn random_range(&self, m: i64, n: i64) -> i64 {
        0
    }
    /// No-op in Factorio; use `LuaRandomGenerator` for custom seeding.
    #[allow(unused_variables)]
    pub fn randomseed(&self, seed: i64) {}
    #[allow(unused_variables)]
    pub fn sin(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn sinh(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn sqrt(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn tan(&self, x: f64) -> f64 {
        0.0
    }
    #[allow(unused_variables)]
    pub fn tanh(&self, x: f64) -> f64 {
        0.0
    }
}

/// Lua `string` library (includes Factorio's `pack` / `packsize` / `unpack`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LuaStringLib;

impl LuaStringLib {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[allow(unused_variables)]
    pub fn byte(&self, s: &'static str, i: Option<i64>, j: Option<i64>) -> Option<u8> {
        None
    }
    #[allow(unused_variables)]
    pub fn char(&self, bytes: impl Into<crate::LuaAny>) -> &'static str {
        ""
    }
    #[allow(unused_variables)]
    pub fn find(
        &self,
        s: &'static str,
        pattern: &'static str,
        init: Option<i64>,
        plain: Option<bool>,
    ) -> Option<(i64, i64)> {
        None
    }
    /// `string.format(fmt)` with no values.
    #[allow(unused_variables)]
    pub fn format(&self, fmt: &'static str) -> &'static str {
        ""
    }
    /// `string.format(fmt, a)`. Lowers as `string.format`.
    #[allow(unused_variables)]
    pub fn format_1(&self, fmt: &'static str, a: impl Into<crate::LuaAny>) -> &'static str {
        ""
    }
    /// `string.format(fmt, a, b)`. Lowers as `string.format`.
    #[allow(unused_variables)]
    pub fn format_2(
        &self,
        fmt: &'static str,
        a: impl Into<crate::LuaAny>,
        b: impl Into<crate::LuaAny>,
    ) -> &'static str {
        ""
    }
    /// `string.format(fmt, a, b, c)`. Lowers as `string.format`.
    #[allow(unused_variables)]
    pub fn format_3(
        &self,
        fmt: &'static str,
        a: impl Into<crate::LuaAny>,
        b: impl Into<crate::LuaAny>,
        c: impl Into<crate::LuaAny>,
    ) -> &'static str {
        ""
    }
    /// `string.format(fmt, a, b, c, d)`. Lowers as `string.format`.
    #[allow(unused_variables)]
    pub fn format_4(
        &self,
        fmt: &'static str,
        a: impl Into<crate::LuaAny>,
        b: impl Into<crate::LuaAny>,
        c: impl Into<crate::LuaAny>,
        d: impl Into<crate::LuaAny>,
    ) -> &'static str {
        ""
    }
    #[allow(unused_variables)]
    pub fn gmatch(&self, s: &'static str, pattern: &'static str) -> crate::LuaAny {
        crate::LuaAny
    }
    #[allow(unused_variables)]
    pub fn gsub(
        &self,
        s: &'static str,
        pattern: &'static str,
        repl: impl Into<crate::LuaAny>,
        n: Option<i64>,
    ) -> (&'static str, i64) {
        ("", 0)
    }
    #[allow(unused_variables)]
    pub fn len(&self, s: &'static str) -> i64 {
        0
    }
    #[allow(unused_variables)]
    pub fn lower(&self, s: &'static str) -> &'static str {
        ""
    }
    #[allow(unused_variables)]
    pub fn r#match(
        &self,
        s: &'static str,
        pattern: &'static str,
        init: Option<i64>,
    ) -> Option<&'static str> {
        None
    }
    /// Binary pack (Lua 5.4 backport in Factorio).
    #[allow(unused_variables)]
    pub fn pack(&self, fmt: &'static str, values: impl Into<crate::LuaAny>) -> &'static str {
        ""
    }
    #[allow(unused_variables)]
    pub fn packsize(&self, fmt: &'static str) -> i64 {
        0
    }
    #[allow(unused_variables)]
    pub fn rep(&self, s: &'static str, n: i64, sep: Option<&'static str>) -> &'static str {
        ""
    }
    #[allow(unused_variables)]
    pub fn reverse(&self, s: &'static str) -> &'static str {
        ""
    }
    #[allow(unused_variables)]
    pub fn sub(&self, s: &'static str, i: i64, j: Option<i64>) -> &'static str {
        ""
    }
    /// Binary unpack (Lua 5.4 backport in Factorio).
    #[allow(unused_variables)]
    pub fn unpack(&self, fmt: &'static str, s: &'static str, pos: Option<i64>) -> crate::LuaAny {
        crate::LuaAny
    }
    #[allow(unused_variables)]
    pub fn upper(&self, s: &'static str) -> &'static str {
        ""
    }
}

/// Lua `table` library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LuaTableLib;

impl LuaTableLib {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[allow(unused_variables)]
    pub fn concat(
        &self,
        list: impl Into<crate::LuaAny>,
        sep: Option<&'static str>,
        i: Option<i64>,
        j: Option<i64>,
    ) -> &'static str {
        ""
    }
    #[allow(unused_variables)]
    pub fn insert(&self, list: impl Into<crate::LuaAny>, value: impl Into<crate::LuaAny>) {}
    /// `table.insert(list, pos, value)`. Lowers as `table.insert`.
    #[allow(unused_variables)]
    pub fn insert_at(
        &self,
        list: impl Into<crate::LuaAny>,
        pos: i64,
        value: impl Into<crate::LuaAny>,
    ) {
    }
    #[allow(unused_variables)]
    pub fn pack(&self, values: impl Into<crate::LuaAny>) -> crate::LuaAny {
        crate::LuaAny
    }
    #[allow(unused_variables)]
    pub fn remove(&self, list: impl Into<crate::LuaAny>, pos: Option<i64>) -> crate::LuaAny {
        crate::LuaAny
    }
    #[allow(unused_variables)]
    pub fn sort(&self, list: impl Into<crate::LuaAny>, comp: Option<crate::LuaFunction>) {}
    #[allow(unused_variables)]
    pub fn unpack(
        &self,
        list: impl Into<crate::LuaAny>,
        i: Option<i64>,
        j: Option<i64>,
    ) -> crate::LuaAny {
        crate::LuaAny
    }
}
