//! 编译期 guard：`FileSystemProvider` trait 不得引入分页 / 排序语义参数。
//!
//! 硬约束 H5：fs trait 只承担"原子 fs 操作"，分页 / 排序 / 取前 N 走更高层
//! （`MetadataCache::list_sessions_by_mtime` 等）。详 `openspec/changes/unify-fs-abstraction/design.md`
//! D2 决策依据：trait 暴露分页参数会让 SSH / HTTP provider 被迫承担状态、缓存与
//! 排序逻辑，违反 fs-abstraction capability 的关注点分离。
//!
//! 实现策略：用 `syn` parse `crates/cdt-fs/src/provider.rs` 源码，遍历 trait 方法
//! 签名，对每个参数的**类型**（不含参数名）递归提取所有 ident，匹配违禁词集合。
//! syn AST 解析天然只看代码不看注释，doc-comment / 行注释里出现违禁词不会误伤。

use syn::{
    File, FnArg, GenericArgument, Item, ItemTrait, PathArguments, TraitItem, TraitItemFn, Type,
};

/// 违禁 type ident：暗示 fs trait 承担分页 / 排序语义。
const FORBIDDEN_IDENTS: &[&str] = &["Cursor", "Offset", "Limit", "SortBy", "Order"];

#[test]
fn file_system_provider_trait_has_no_pagination_or_sorting_parameters() {
    let source = include_str!("../src/provider.rs");
    let file: File = syn::parse_str(source).expect("provider.rs 必须能被 syn 解析");

    let trait_item = find_trait(&file, "FileSystemProvider")
        .expect("provider.rs 必须含 `trait FileSystemProvider`");

    let violations = collect_violations(trait_item);

    assert!(
        violations.is_empty(),
        "FileSystemProvider trait 引入了分页 / 排序参数类型：\n{}\n\n\
         违反 H5：fs trait 不承担分页 / 排序语义（按 mtime 拿前 N 走更高层）。\n\
         详 openspec/changes/unify-fs-abstraction/design.md D2 决策依据。",
        format_violations(&violations),
    );
}

#[test]
fn doc_comment_with_forbidden_word_does_not_trigger_false_positive() {
    // negative test：trait 的 doc-comment 含 `Cursor` 字面量，但方法签名干净 → 必须 pass。
    let source = r"
        use async_trait::async_trait;

        #[async_trait]
        pub trait FileSystemProvider: Send + Sync + 'static {
            /// example: don't add `Cursor` parameter to this trait.
            /// 反例：`Offset` / `Limit` / `SortBy` / `Order` 都不应出现在签名里。
            // line comment: Cursor Offset Limit SortBy Order
            async fn read_to_string(&self, path: &str) -> Result<String, ()>;
        }
    ";
    let file: File = syn::parse_str(source).expect("inline 源必须能被 syn 解析");
    let trait_item = find_trait(&file, "FileSystemProvider").expect("inline trait 必须存在");
    let violations = collect_violations(trait_item);
    assert!(
        violations.is_empty(),
        "negative test 误伤注释中的违禁词：{violations:?}"
    );
}

#[test]
fn positive_control_detects_synthetic_cursor_parameter() {
    // positive control：保证检测逻辑真的会命中——避免规则失效但测试常绿。
    let source = r"
        use async_trait::async_trait;

        pub struct Cursor;

        #[async_trait]
        pub trait FileSystemProvider: Send + Sync + 'static {
            async fn list(&self, cursor: Cursor) -> Result<(), ()>;
        }
    ";
    let file: File = syn::parse_str(source).expect("inline 源必须能被 syn 解析");
    let trait_item = find_trait(&file, "FileSystemProvider").expect("inline trait 必须存在");
    let violations = collect_violations(trait_item);
    assert_eq!(violations.len(), 1, "positive control 必须命中 1 个违规");
    assert_eq!(violations[0].method, "list");
    assert_eq!(violations[0].forbidden_ident, "Cursor");
}

#[derive(Debug)]
struct Violation {
    method: String,
    /// 参数名（pat），仅用于 panic 消息定位；检测本身只看 type ident。
    param_name: String,
    forbidden_ident: String,
}

fn find_trait<'a>(file: &'a File, name: &str) -> Option<&'a ItemTrait> {
    file.items.iter().find_map(|item| match item {
        Item::Trait(t) if t.ident == name => Some(t),
        _ => None,
    })
}

fn collect_violations(trait_item: &ItemTrait) -> Vec<Violation> {
    let mut out = Vec::new();
    for item in &trait_item.items {
        let TraitItem::Fn(method) = item else {
            continue;
        };
        scan_method(method, &mut out);
    }
    out
}

fn scan_method(method: &TraitItemFn, out: &mut Vec<Violation>) {
    let method_name = method.sig.ident.to_string();
    for arg in &method.sig.inputs {
        let FnArg::Typed(pat_type) = arg else {
            // `&self` / `&mut self` —— 不带类型参数，跳过
            continue;
        };
        let mut idents = Vec::new();
        collect_type_idents(&pat_type.ty, &mut idents);
        let param_name = pat_to_string(&pat_type.pat);
        for ident in idents {
            if let Some(&hit) = FORBIDDEN_IDENTS.iter().find(|f| **f == ident) {
                out.push(Violation {
                    method: method_name.clone(),
                    param_name: param_name.clone(),
                    forbidden_ident: hit.to_string(),
                });
            }
        }
    }
}

/// 递归提取一个 `Type` 内出现的所有 path-segment ident。
///
/// 覆盖：`Type::Path`（普通命名类型 + 泛型实参）/ `Type::Reference`（`&T` / `&mut T`）/
/// `Type::Slice`（`[T]`）/ `Type::Array`（`[T; N]`）/ `Type::Tuple`（`(T1, T2)`）/
/// `Type::Paren` / `Type::Group` / `Type::Ptr` / `Type::TraitObject`（`dyn Trait`）/
/// `Type::ImplTrait`（`impl Trait`）。其他 variant（`Type::BareFn` / `Type::Macro` 等）
/// 当前 trait 签名不会出现，遇到忽略即可。
fn collect_type_idents(ty: &Type, out: &mut Vec<String>) {
    match ty {
        Type::Path(type_path) => {
            for segment in &type_path.path.segments {
                out.push(segment.ident.to_string());
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let GenericArgument::Type(inner) = arg {
                            collect_type_idents(inner, out);
                        }
                    }
                }
            }
        }
        Type::Reference(r) => collect_type_idents(&r.elem, out),
        Type::Slice(s) => collect_type_idents(&s.elem, out),
        Type::Array(a) => collect_type_idents(&a.elem, out),
        Type::Tuple(t) => {
            for elem in &t.elems {
                collect_type_idents(elem, out);
            }
        }
        Type::Paren(p) => collect_type_idents(&p.elem, out),
        Type::Group(g) => collect_type_idents(&g.elem, out),
        Type::Ptr(p) => collect_type_idents(&p.elem, out),
        Type::TraitObject(obj) => {
            for bound in &obj.bounds {
                if let syn::TypeParamBound::Trait(tb) = bound {
                    for segment in &tb.path.segments {
                        out.push(segment.ident.to_string());
                    }
                }
            }
        }
        Type::ImplTrait(it) => {
            for bound in &it.bounds {
                if let syn::TypeParamBound::Trait(tb) = bound {
                    for segment in &tb.path.segments {
                        out.push(segment.ident.to_string());
                    }
                }
            }
        }
        _ => {}
    }
}

fn pat_to_string(pat: &syn::Pat) -> String {
    match pat {
        syn::Pat::Ident(i) => i.ident.to_string(),
        // 其他 pat（解构、通配等）当前 trait 签名不会出现；万一以后用到，
        // 退化成占位字符串即可，定位由方法名 + 违禁 ident 已足够。
        _ => "<non-ident-pat>".to_string(),
    }
}

fn format_violations(violations: &[Violation]) -> String {
    violations
        .iter()
        .map(|v| {
            format!(
                "  - 方法 `{}` 的参数 `{}` 类型含违禁 ident `{}`",
                v.method, v.param_name, v.forbidden_ident,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
