use syn::{Fields, Type, Visibility};

use crate::error::{FrontendError, FrontendResult};

use super::{
    types::{TypeAlias, lower_type, type_source_string},
    util::location,
};

pub struct PendingStruct {
    pub visibility: Visibility,
    pub fields: Vec<factorio_ir::structure::StructField>,
    pub constants: Vec<(String, factorio_ir::expression::Expression)>,
    pub methods: Vec<factorio_ir::function::Function>,
    pub doc: Option<String>,
}

pub struct PendingEnum {
    pub visibility: Visibility,
    pub variants: Vec<factorio_ir::enumeration::EnumVariant>,
    pub constants: Vec<(String, factorio_ir::expression::Expression)>,
    pub methods: Vec<factorio_ir::function::Function>,
    pub doc: Option<String>,
}

impl PendingEnum {
    pub const fn new(visibility: Visibility) -> Self {
        Self {
            visibility,
            variants: Vec::new(),
            constants: Vec::new(),
            methods: Vec::new(),
            doc: None,
        }
    }
}

impl PendingStruct {
    pub const fn new(visibility: Visibility) -> Self {
        Self {
            visibility,
            fields: Vec::new(),
            constants: Vec::new(),
            methods: Vec::new(),
            doc: None,
        }
    }
}

pub fn lower_struct_fields(
    fields: &Fields,
    aliases: &std::collections::HashMap<String, TypeAlias>,
) -> FrontendResult<Vec<factorio_ir::structure::StructField>> {
    match fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .map(|field| {
                let name = field.ident.as_ref().ok_or_else(|| {
                    FrontendError::ExpectedIdentifierPattern {
                        location: location(field),
                    }
                })?;
                Ok(factorio_ir::structure::StructField {
                    name: name.to_string(),
                    ty: lower_type(&field.ty, aliases)?,
                    source_type: Some(type_source_string(&field.ty, aliases)),
                })
            })
            .collect(),
        Fields::Unit => Ok(Vec::new()),
        Fields::Unnamed(_) => Err(FrontendError::UnsupportedItem {
            item: "tuple struct".to_string(),
            location: location(fields),
        }),
    }
}

pub fn impl_type_name(ty: &Type) -> FrontendResult<String> {
    match ty {
        Type::Path(path) if path.path.segments.len() == 1 => {
            Ok(path.path.segments[0].ident.to_string())
        }
        _ => Err(FrontendError::UnsupportedType {
            ty: "unsupported impl type".to_string(),
            location: location(ty),
        }),
    }
}
