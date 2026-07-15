use crate::ast::{Declaration, Expression, Span, Statement};
use crate::codegen::compiler::Compiler;
use inkwell::types::BasicTypeEnum;
use inkwell::values::PointerValue;
use std::collections::HashMap;

impl<'ctx> Compiler<'ctx> {
    /// Type-recovery helpers used by `compile_expr` and `compile_block` to
    /// reconstruct the Aion type-name string of an expression / field when
    /// only the LLVM value or the surrounding struct type-name is available.
    /// Phase 7 of the `compiler.rs` split (#113): moved verbatim from
    /// `src/codegen/compiler.rs` — behaviour-preserving code motion. The
    /// string-based recovery here is the subject of #108 (replace with a
    /// real `Type` in variable slots); this phase only relocates it.
    pub(in crate::codegen) fn get_field_type(&self, it: &str, fnm: &str) -> String {
        let clean = it.replace(" ", "");
        let mut cs = clean.as_str();
        while cs.starts_with('*') {
            cs = &cs[1..];
        }
        // Tuple field access by numeric index: `(i64,String)` + "0" -> "i64".
        // Depth-aware split so nested tuples work. #53.
        if cs.starts_with('(') && cs.ends_with(')') {
            if let Ok(idx) = fnm.parse::<usize>() {
                let inner = &cs[1..cs.len() - 1];
                let mut depth = 0i32;
                let mut cur = String::new();
                let mut parts: Vec<String> = Vec::new();
                for c in inner.chars() {
                    match c {
                        '(' | '<' => {
                            depth += 1;
                            cur.push(c);
                        }
                        ')' | '>' => {
                            depth -= 1;
                            cur.push(c);
                        }
                        ',' if depth == 0 => {
                            parts.push(cur.clone());
                            cur.clear();
                        }
                        _ => cur.push(c),
                    }
                }
                parts.push(cur);
                if idx < parts.len() {
                    return parts[idx].trim().to_string();
                }
            }
            return "unknown".to_string();
        }
        let (btn, tga) = if cs.contains('<') {
            let p: Vec<&str> = cs
                .split(['<', '>', ','])
                .filter(|s| !s.is_empty())
                .collect();
            (
                p[0].to_string(),
                p[1..]
                    .iter()
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<String>>(),
            )
        } else {
            (cs.to_string(), vec![])
        };
        let full = self
            .resolve_fuzzy_name(&self.struct_types, &btn)
            .unwrap_or(btn);
        if let Some(Declaration::Struct(s)) = self.decls.get(&full) {
            for (f_nm, ft) in &s.fields {
                if f_nm == fnm {
                    let rft = Self::substitute_generic_params(ft, &s.generic_params, &tga);
                    return rft.replace(" ", "");
                }
            }
        }
        "unknown".to_string()
    }
    pub(in crate::codegen) fn get_expr_type_name(
        &self,
        e: &Expression,
        variables: &HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>, String)>,
    ) -> String {
        let res = match e {
            Expression::Integer(_, _) => "i64".to_string(),
            Expression::Float(_, _) => "f64".to_string(),
            Expression::Char(_, _) => "i64".to_string(),
            Expression::Boolean(_, _) => "bool".to_string(),
            Expression::String(_, _) => "String".to_string(),
            Expression::Identifier(name, _) => {
                let parts: Vec<&str> = name.split('.').collect();
                if parts.len() > 1 {
                    let mut ct = if let Some((_, _, t)) = variables.get(parts[0]) {
                        t.clone()
                    } else {
                        "unknown".to_string()
                    };
                    for p in parts.iter().skip(1) {
                        if ct == "unknown" {
                            break;
                        }
                        ct = self.get_field_type(&ct, p);
                    }
                    return ct.replace(" ", "");
                }
                if let Some((_, _, t)) = variables.get(name) {
                    return t.clone().replace(" ", "");
                }
                // Bare function name used as a value → recover its signature
                // from the declaration so `let f = double` stores a
                // recognizable `fn(...)->...` type-name. #84.
                if let Some(Declaration::Function(f)) = self.decls.get(name)
                    && f.generic_params.is_empty()
                {
                    let params: Vec<String> =
                        f.params.iter().map(|(_, pt, _)| pt.clone()).collect();
                    return format!("fn({})->{}", params.join(","), f.return_type).replace(" ", "");
                }
                "unknown".to_string()
            }
            Expression::Call {
                function: name,
                generic_args,
                ..
            } => {
                if let Some((rn, mn)) = name.rsplit_once('.') {
                    if mn == "offset" {
                        let rt = self.get_expr_type_name(
                            &Expression::Identifier(rn.to_string(), Span::zero()),
                            variables,
                        );
                        if rt.starts_with('*') {
                            return rt.replace(" ", "");
                        }
                    }
                    let rt = self.get_expr_type_name(
                        &Expression::Identifier(rn.to_string(), Span::zero()),
                        variables,
                    );
                    if rt != "unknown" {
                        let (btn, tga) = if rt.contains('<') {
                            let p: Vec<&str> = rt
                                .split(['<', '>', ','])
                                .filter(|s| !s.is_empty())
                                .collect();
                            (
                                p[0].to_string(),
                                p[1..]
                                    .iter()
                                    .map(|s| s.trim().to_string())
                                    .collect::<Vec<String>>(),
                            )
                        } else {
                            (rt.clone(), vec![])
                        };
                        let mut bc = btn.as_str();
                        while bc.starts_with('*') {
                            bc = &bc[1..];
                        }
                        let tp = self
                            .resolve_fuzzy_name(&self.struct_types, bc)
                            .or_else(|| self.resolve_fuzzy_name(&self.enum_types, bc))
                            .unwrap_or(btn.clone());
                        let method_name_colon = format!("{}::{}", tp, mn);
                        let method_name_dot = format!("{}.{}", tp, mn);
                        if let Some(Declaration::Function(f_decl)) = self
                            .decls
                            .get(&method_name_colon)
                            .or_else(|| self.decls.get(&method_name_dot))
                        {
                            let mut res_type = f_decl.return_type.clone();
                            let combined_args = if generic_args.is_empty() {
                                &tga
                            } else {
                                generic_args
                            };
                            res_type = Self::substitute_generic_params(
                                &res_type,
                                &f_decl.generic_params,
                                combined_args,
                            );
                            return res_type.replace(" ", "");
                        }
                    }
                }
                let full = self
                    .resolve_fuzzy_name(&self.decls, name)
                    .unwrap_or(name.clone());
                if let Some(Declaration::Function(f_decl)) = self.decls.get(&full) {
                    let r = Self::substitute_generic_params(
                        &f_decl.return_type,
                        &f_decl.generic_params,
                        generic_args,
                    );
                    return r.replace(" ", "");
                }
                "unknown".to_string()
            }
            Expression::MemberAccess {
                receiver, member, ..
            } => {
                let rt = self.get_expr_type_name(receiver, variables);
                self.get_field_type(&rt, member)
            }
            Expression::MethodCall {
                receiver,
                method,
                generic_args,
                ..
            } => {
                let rt = self.get_expr_type_name(receiver, variables);
                if method == "offset" && rt.starts_with('*') {
                    return rt.clone();
                }
                let (btn, tga) = if rt.contains('<') {
                    let p: Vec<&str> = rt
                        .split(['<', '>', ','])
                        .filter(|s| !s.is_empty())
                        .collect();
                    (
                        p[0].to_string(),
                        p[1..]
                            .iter()
                            .map(|s| s.trim().to_string())
                            .collect::<Vec<String>>(),
                    )
                } else {
                    (rt.clone(), vec![])
                };
                let mut bc = btn.as_str();
                while bc.starts_with('*') {
                    bc = &bc[1..];
                }
                let full = self
                    .resolve_fuzzy_name(&self.decls, bc)
                    .unwrap_or(btn.clone());
                let method_name_colon = format!("{}::{}", full, method);
                let method_name_dot = format!("{}.{}", full, method);
                if let Some(Declaration::Function(f_decl)) = self
                    .decls
                    .get(&method_name_colon)
                    .or_else(|| self.decls.get(&method_name_dot))
                {
                    let mut res_type = Self::substitute_generic_params(
                        &f_decl.return_type,
                        &f_decl.generic_params,
                        generic_args,
                    );
                    if let Some(decl) = self.decls.get(&full) {
                        let bp = match decl {
                            Declaration::Struct(s) => &s.generic_params,
                            Declaration::Enum(e) => &e.generic_params,
                            _ => &vec![],
                        };
                        res_type = Self::substitute_generic_params(&res_type, bp, &tga);
                    }
                    return res_type.replace(" ", "");
                }
                "unknown".to_string()
            }
            Expression::StructInst {
                name, generic_args, ..
            } => {
                let sn = self
                    .resolve_fuzzy_name(&self.struct_types, name)
                    .unwrap_or(name.clone());
                if generic_args.is_empty() {
                    sn.replace(" ", "")
                } else {
                    format!("{}<{}>", sn, generic_args.join(",")).replace(" ", "")
                }
            }
            Expression::EnumInst {
                name,
                variant,
                arguments,
                generic_args,
                ..
            } => {
                if let Some(en) = self.resolve_fuzzy_name(&self.enum_types, name) {
                    if !generic_args.is_empty() {
                        format!("{}<{}>", en, generic_args.join(",")).replace(" ", "")
                    } else {
                        // Infer generic args from variant arguments
                        let mut inferred_args: Vec<String> = vec![];
                        if let Some(Declaration::Enum(_)) = self.decls.get(&en) {
                            for arg in arguments.iter() {
                                inferred_args.push(self.get_expr_type_name(arg, variables));
                            }
                        }
                        if inferred_args.is_empty() {
                            en.replace(" ", "")
                        } else {
                            format!("{}<{}>", en, inferred_args.join(",")).replace(" ", "")
                        }
                    }
                } else {
                    let call_expr = Expression::Call {
                        function: format!("{}.{}", name, variant),
                        generic_args: generic_args.clone(),
                        arguments: arguments.clone(),
                        span: Span::zero(),
                    };
                    self.get_expr_type_name(&call_expr, variables)
                }
            }
            Expression::Cast { target, .. } => target.replace(" ", ""),
            Expression::Block { statements, .. } => {
                if let Some(s) = statements.last() {
                    match s {
                        Statement::ExpressionStmt(e, _) | Statement::Return { value: e, .. } => {
                            self.get_expr_type_name(e, variables)
                        }
                        _ => "unknown".to_string(),
                    }
                } else {
                    "unknown".to_string()
                }
            }
            Expression::Deref { expr, .. } => {
                let t = self.get_expr_type_name(expr, variables);
                if let Some(stripped) = t.strip_prefix('*') {
                    stripped.to_string().replace(" ", "")
                } else {
                    "unknown".to_string()
                }
            }
            Expression::TypeRef {
                name, generic_args, ..
            } => {
                if generic_args.is_empty() {
                    name.clone()
                } else {
                    format!("{}<{}>", name, generic_args.join(","))
                }
            }
            Expression::Match { arms, .. } => {
                if let Some(arm) = arms.first() {
                    if let Some(s) = arm.body.last() {
                        match s {
                            Statement::ExpressionStmt(e, _)
                            | Statement::Return { value: e, .. } => {
                                self.get_expr_type_name(e, variables)
                            }
                            _ => "unknown".to_string(),
                        }
                    } else {
                        "unknown".to_string()
                    }
                } else {
                    "unknown".to_string()
                }
            }
            Expression::TupleLiteral { elements, .. } => {
                // Render the tuple type name `(T, U, ...)` so downstream
                // lookups can find the registered struct type. #53.
                let elems: Vec<String> = elements
                    .iter()
                    .map(|e| self.get_expr_type_name(e, variables))
                    .collect();
                format!("({})", elems.join(","))
            }
            Expression::TupleAccess { tuple, index, .. } => {
                // The element type name of field `index`. #53.
                let tn = self.get_expr_type_name(tuple, variables);
                self.get_field_type(&tn, &index.to_string())
            }
            Expression::ArrayLiteral { elements, .. } => {
                // Render `[T; N]` from the first element's type so that
                // downstream `arr[i]` codegen can parse the array type. #54.
                let elem_tn = if elements.is_empty() {
                    "i64".to_string()
                } else {
                    self.get_expr_type_name(&elements[0], variables)
                };
                format!("[{}; {}]", elem_tn, elements.len())
            }
            _ => "unknown".to_string(),
        };
        res.replace(" ", "")
    }
}
