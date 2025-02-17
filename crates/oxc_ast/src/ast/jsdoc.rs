//! [`JSDoc`](https://github.com/microsoft/TypeScript/blob/54a554d8af2657630307cbfa8a3e4f3946e36507/src/compiler/types.ts#L393)

use oxc_span::Span;
#[cfg(feature = "serde")]
use serde::Serialize;

use crate::ast::TSType;

#[derive(Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize), serde(tag = "type", rename_all = "camelCase"))]
#[cfg_attr(all(feature = "serde", feature = "wasm"), derive(tsify::Tsify))]
pub struct JSDocNullableType<'a> {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub span: Span,
    pub type_annotation: TSType<'a>,
    pub postfix: bool,
}

#[derive(Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize), serde(tag = "type", rename_all = "camelCase"))]
#[cfg_attr(all(feature = "serde", feature = "wasm"), derive(tsify::Tsify))]
pub struct JSDocUnknownType {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub span: Span,
}
