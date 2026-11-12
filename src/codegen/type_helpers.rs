use crate::analysis::types::Type;
use crate::ast::{Declaration, Expression, Span, Statement};
use crate::codegen::compiler::{Compiler, VarMap};

impl<'ctx> Compiler<'ctx> {
    /// Structural type recovery for codegen (#108).
    ///
    /// Variable slots store a `crate::analysis::types::Type` directly instead
    /// of a formatted `String`, eliminating ad-hoc string formatting and
    /// re-parsing across tuples, arrays, pointers, and generic instances.
    /// Adding a new expression or type variant only requires returning its
    /// `Type` in `get_expr_type` — no string round-trip helpers are needed.
    pub(in crate::codegen) fn resolve_field_type(&self, it: &Type, fnm: &str) -> Type {
        let mut base = it;
        while let Type::Pointer(inner) = base {
            base = inner;
        }
        // Tuple field access by numeric index: `(T, U, ...)` + "0" -> `T`. #53, #108.
        if let Type::Tuple(elems) = base {
            if let Ok(idx) = fnm.parse::<usize>()
                && let Some(elem) = elems.get(idx)
            {
                return elem.clone();
            }
            return Type::Unknown;
        }
        let (btn, tga): (String, Vec<String>) = match base {
            Type::GenericInstance(name, args) => (
                name.clone(),
                args.iter().map(|a| a.name().replace(" ", "")).collect(),
            ),
            Type::Struct { name } | Type::Enum { name } | Type::Placeholder(name) => {
                (name.clone(), vec![])
            }
            _ => return Type::Unknown,
        };
        let full = self
            .resolve_fuzzy_name(&self.struct_types, &btn)
            .unwrap_or(btn);
        if let Some(Declaration::Struct(s)) = self.decls.get(&full) {
            for (f_nm, ft) in &s.fields {
                if f_nm == fnm {
                    let rft = Self::substitute_generic_params(ft, &s.generic_params, &tga);
                    return Type::parse(&rft);
                }
            }
        }
        Type::Unknown
    }

    /// Decompose a receiver `Type` into `(base_type_name, generic_arg_strings)`
    /// after stripping any leading `Type::Pointer` layers.
    pub(in crate::codegen) fn decompose_receiver_type(&self, ty: &Type) -> (String, Vec<String>) {
        let mut base = ty;
        while let Type::Pointer(inner) = base {
            base = inner;
        }
        match base {
            Type::GenericInstance(name, args) => (
                name.clone(),
                args.iter().map(|a| a.name().replace(" ", "")).collect(),
            ),
            Type::Struct { name } | Type::Enum { name } | Type::Placeholder(name) => {
                (name.clone(), vec![])
            }
            other => (other.name().replace(" ", ""), vec![]),
        }
    }

    /// Recover the structural `Type` of an `Expression` from the current
    /// `VarMap` and declaration table (#108).
    pub(in crate::codegen) fn get_expr_type(
        &self,
        e: &Expression,
        variables: &VarMap<'ctx>,
    ) -> Type {
        match e {
            Expression::Integer(_, _) | Expression::Char(_, _) => Type::i64(),
            Expression::Float(_, _) => Type::Float,
            Expression::Boolean(_, _) => Type::Boolean,
            Expression::String(_, _) => Type::String,
            Expression::Duration(_, _, _) => Type::Duration,
            Expression::Date(_, _) => Type::Date,
            Expression::Infix {
                left,
                operator,
                right,
                ..
            } => {
                use crate::lexer::token::TokenKind;
                match operator.kind {
                    TokenKind::EqEq
                    | TokenKind::NotEq
                    | TokenKind::Lt
                    | TokenKind::Gt
                    | TokenKind::LtEq
                    | TokenKind::GtEq
                    | TokenKind::And
                    | TokenKind::Or
                    | TokenKind::Inside => Type::Boolean,
                    TokenKind::Plus => {
                        let lt = self.get_expr_type(left, variables);
                        if lt == Type::String {
                            Type::String
                        } else {
                            let rt = self.get_expr_type(right, variables);
                            if rt == Type::String {
                                Type::String
                            } else if lt == Type::Float || rt == Type::Float {
                                Type::Float
                            } else if lt != Type::Unknown {
                                lt
                            } else {
                                Type::i64()
                            }
                        }
                    }
                    _ => {
                        let lt = self.get_expr_type(left, variables);
                        if lt != Type::Unknown { lt } else { Type::i64() }
                    }
                }
            }
            Expression::Identifier(name, _) => {
                let parts: Vec<&str> = name.split('.').collect();
                if parts.len() > 1 {
                    let mut ct = if let Some((_, _, t)) = variables.get(parts[0]) {
                        t.clone()
                    } else {
                        Type::Unknown
                    };
                    for p in parts.iter().skip(1) {
                        if ct == Type::Unknown {
                            break;
                        }
                        ct = self.resolve_field_type(&ct, p);
                    }
                    return ct;
                }
                if let Some((_, _, t)) = variables.get(name) {
                    return t.clone();
                }
                // Bare function name used as a value → recover its signature
                // from the declaration so `let f = double` stores a structural
                // `Type::Function`. #84, #108.
                if let Some(Declaration::Function(f)) = self.decls.get(name)
                    && f.generic_params.is_empty()
                {
                    let params: Vec<Type> =
                        f.params.iter().map(|(_, pt, _)| Type::parse(pt)).collect();
                    return Type::Function {
                        is_unsafe: false,
                        params,
                        return_type: Box::new(Type::parse(&f.return_type)),
                    };
                }
                Type::Unknown
            }
            Expression::Call {
                function: name,
                generic_args,
                ..
            } => {
                if let Some((rn, mn)) = name.rsplit_once('.') {
                    let rt = self.get_expr_type(
                        &Expression::Identifier(rn.to_string(), Span::zero()),
                        variables,
                    );
                    if mn == "offset" && matches!(rt, Type::Pointer(_)) {
                        return rt;
                    }
                    if rt != Type::Unknown {
                        let (btn, tga) = self.decompose_receiver_type(&rt);
                        let tp = self
                            .resolve_fuzzy_name(&self.struct_types, &btn)
                            .or_else(|| self.resolve_fuzzy_name(&self.enum_types, &btn))
                            .unwrap_or(btn);
                        let method_name_colon = format!("{}::{}", tp, mn);
                        let method_name_dot = format!("{}.{}", tp, mn);
                        if let Some(Declaration::Function(f_decl)) = self
                            .decls
                            .get(&method_name_colon)
                            .or_else(|| self.decls.get(&method_name_dot))
                        {
                            let combined_args = if generic_args.is_empty() {
                                &tga
                            } else {
                                generic_args
                            };
                            let res_type = Self::substitute_generic_params(
                                &f_decl.return_type,
                                &f_decl.generic_params,
                                combined_args,
                            );
                            return Type::parse(&res_type);
                        }
                    }
                }
                if let Some((_, _, Type::Function { return_type, .. })) = variables.get(name) {
                    return (**return_type).clone();
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
                    return Type::parse(&r);
                }
                Type::Unknown
            }
            Expression::MemberAccess {
                receiver, member, ..
            } => {
                let rt = self.get_expr_type(receiver, variables);
                self.resolve_field_type(&rt, member)
            }
            Expression::MethodCall {
                receiver,
                method,
                generic_args,
                ..
            } => {
                let rt = self.get_expr_type(receiver, variables);
                if method == "offset" && matches!(rt, Type::Pointer(_)) {
                    return rt;
                }
                let (btn, tga) = self.decompose_receiver_type(&rt);
                let full = self.resolve_fuzzy_name(&self.decls, &btn).unwrap_or(btn);
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
                    return Type::parse(&res_type);
                }
                Type::Unknown
            }
            Expression::StructInst {
                name, generic_args, ..
            } => {
                let sn = self
                    .resolve_fuzzy_name(&self.struct_types, name)
                    .unwrap_or(name.clone());
                if generic_args.is_empty() {
                    Type::Struct { name: sn }
                } else {
                    Type::GenericInstance(sn, generic_args.iter().map(|g| Type::parse(g)).collect())
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
                        Type::GenericInstance(
                            en,
                            generic_args.iter().map(|g| Type::parse(g)).collect(),
                        )
                    } else {
                        let mut inferred_args: Vec<Type> = Vec::new();
                        if let Some(Declaration::Enum(_)) = self.decls.get(&en) {
                            for arg in arguments.iter() {
                                inferred_args.push(self.get_expr_type(arg, variables));
                            }
                        }
                        if inferred_args.is_empty() {
                            Type::Enum { name: en }
                        } else {
                            Type::GenericInstance(en, inferred_args)
                        }
                    }
                } else {
                    let call_expr = Expression::Call {
                        function: format!("{}.{}", name, variant),
                        generic_args: generic_args.clone(),
                        arguments: arguments.clone(),
                        span: Span::zero(),
                    };
                    self.get_expr_type(&call_expr, variables)
                }
            }
            Expression::Cast { target, .. } => Type::parse(target),
            Expression::Block { statements, .. } => {
                if let Some(Statement::ExpressionStmt(e, _) | Statement::Return { value: e, .. }) =
                    statements.last()
                {
                    self.get_expr_type(e, variables)
                } else {
                    Type::Unknown
                }
            }
            Expression::If {
                then_branch,
                else_branch,
                ..
            } => {
                if let Some(Statement::ExpressionStmt(e, _) | Statement::Return { value: e, .. }) =
                    then_branch.last()
                {
                    let t = self.get_expr_type(e, variables);
                    if t != Type::Unknown {
                        return t;
                    }
                }
                if let Some(eb) = else_branch
                    && let Some(
                        Statement::ExpressionStmt(e, _) | Statement::Return { value: e, .. },
                    ) = eb.last()
                {
                    return self.get_expr_type(e, variables);
                }
                Type::Unknown
            }
            Expression::Deref { expr, .. } => match self.get_expr_type(expr, variables) {
                Type::Pointer(inner) => *inner,
                other => other,
            },
            Expression::TypeRef {
                name, generic_args, ..
            } => {
                if generic_args.is_empty() {
                    Type::parse(name)
                } else {
                    Type::GenericInstance(
                        name.clone(),
                        generic_args.iter().map(|g| Type::parse(g)).collect(),
                    )
                }
            }
            Expression::Match { arms, .. } => {
                if let Some(arm) = arms.first()
                    && let Some(
                        Statement::ExpressionStmt(e, _) | Statement::Return { value: e, .. },
                    ) = arm.body.last()
                {
                    return self.get_expr_type(e, variables);
                }
                Type::Unknown
            }
            Expression::TupleLiteral { elements, .. } => Type::Tuple(
                elements
                    .iter()
                    .map(|el| self.get_expr_type(el, variables))
                    .collect(),
            ),
            Expression::TupleAccess { tuple, index, .. } => {
                let tt = self.get_expr_type(tuple, variables);
                self.resolve_field_type(&tt, &index.to_string())
            }
            Expression::ArrayLiteral { elements, .. } => {
                let elem_ty = elements
                    .first()
                    .map(|el| self.get_expr_type(el, variables))
                    .unwrap_or_else(Type::i64);
                Type::Array(Box::new(elem_ty), elements.len() as u64)
            }
            Expression::Index { target, .. } => match self.get_expr_type(target, variables) {
                Type::Array(elem, _) | Type::Pointer(elem) => *elem,
                _ => Type::Unknown,
            },
            Expression::Try { expr, .. } => match self.get_expr_type(expr, variables) {
                Type::GenericInstance(_, mut args) if !args.is_empty() => args.remove(0),
                _ => Type::i64(),
            },
            _ => Type::Unknown,
        }
    }
}
