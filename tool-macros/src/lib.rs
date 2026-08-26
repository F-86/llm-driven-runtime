//! 为工具实现生成参数 Schema 的过程宏。

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemImpl, Type, parse_macro_input};

/// 为 Tool 的 impl 自动生成 `parameter_schema` 方法。
///
/// 用法：
///
/// ```ignore
/// #[tool_schema(GetRuntimeStatusArg)]
/// impl Tool for GetRuntimeStatus {
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn tool_schema(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut tool_impl = parse_macro_input!(item as ItemImpl);

    // 检查是否已经有 parameter_schema 方法了，如果有就报错
    let already_exists = tool_impl.items.iter().any(|item| {
        let syn::ImplItem::Fn(method) = item else {
            return false;
        };

        method.sig.ident == "parameter_schema"
    });
    if already_exists {
        return syn::Error::new_spanned(tool_impl, "parameter_schema 已经实现，不能重复生成")
            .to_compile_error()
            .into();
    }

    let parameter_type = parse_macro_input!(attr as Type);

    let parameter_schema_method = syn::parse_quote! {
        fn parameter_schema(&self) -> serde_json::Value {
            serde_json::to_value(schemars::schema_for!(#parameter_type))
                .expect("生成参数 schema 失败")
        }
    };

    tool_impl.items.push(parameter_schema_method);

    quote! {
        #tool_impl
    }
    .into()
}
