use crate::ast::{Span, Statement, Expression, Program, Declaration};
use crate::types::Type;
use crate::environment::Environment;
use crate::token::{Token, TokenKind};
use crate::error::CompileError;

use std::collections::HashMap;

pub struct TypeChecker {
    pub env: Environment,
    pub type_params: HashMap<String, Type>, // Mapping from 'T' to concrete type
    pub decls: HashMap<String, Declaration>,
    pub current_module: Option<String>,
    in_unsafe_context: bool,
    source: String,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self::with_source("")
    }

    pub fn with_source(source: &str) -> Self {
        let mut checker = Self { 
            env: Environment::new(), 
            type_params: HashMap::new(),
            decls: HashMap::new(),
            current_module: None,
            in_unsafe_context: false,
            source: source.to_string(),
        };
        checker.register_builtins();
        checker
    }

    fn err(&self, msg: impl Into<String>, expr: &Expression) -> CompileError {
        let span = expr.span();
        CompileError::new(msg, span.line, span.col).with_snippet(&self.source)
    }

    fn resolve_type(&self, name: &str) -> Type {
        if let Some(t) = self.type_params.get(name) {
            return t.clone();
        }
        let trimmed = name.trim();
        if let Some(t) = self.env.get(trimmed) { return t; }
        if let Some(ref module) = self.current_module {
            let full_name = format!("{}.{}", module, trimmed);
            if let Some(t) = self.env.get(&full_name) { return t; }
        }
        Type::from_str(trimmed)
    }

    fn register_builtins(&mut self) {
        self.env.set("aion_read_file".to_string(), Type::Function { is_unsafe: true, return_type: Box::new(Type::String) });
        self.env.set("aion_write_file".to_string(), Type::Function { is_unsafe: true, return_type: Box::new(Type::Integer) });
        self.env.set("aion_get_argc".to_string(), Type::Function { is_unsafe: true, return_type: Box::new(Type::Integer) });
        self.env.set("aion_get_argv_index".to_string(), Type::Function { is_unsafe: true, return_type: Box::new(Type::String) });
        self.env.set("aion_str_ptr".to_string(), Type::Function { is_unsafe: true, return_type: Box::new(Type::Pointer(Box::new(Type::Integer))) });
        self.env.set("exit".to_string(), Type::Function { is_unsafe: true, return_type: Box::new(Type::Unit) });
        self.env.set("aion_exit".to_string(), Type::Function { is_unsafe: true, return_type: Box::new(Type::Unit) });

        self.env.set("io.println".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Unit) });
        self.env.set("io.print".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Unit) });
        self.env.set("env.var".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::GenericInstance("Option".to_string(), vec![Type::String])) });
        self.env.set("mem.is_null".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Boolean) });
        self.env.set("string.len".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Integer) });
        self.env.set("String.len".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Integer) });
        self.env.set("string.concat".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::String) });
        self.env.set("string.from_int".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::String) });
        self.env.set("string.from_float".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::String) });
        self.env.set("string.to_float".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Float) });
        
        // i64 methods as functions
        self.env.set("i64.abs".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Integer) });
        self.env.set("i64.max".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Integer) });
        self.env.set("i64.min".to_string(), Type::Function { is_unsafe: false, return_type: Box::new(Type::Integer) });
        
        self.env.set("argc".to_string(), Type::Integer);
        self.env.set("argv".to_string(), Type::String);
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), CompileError> {
        self.current_module = program.module_name.clone();
        for decl in &program.declarations {
            match decl {
                Declaration::Function(f) => {
                    self.decls.insert(f.name.clone(), decl.clone());
                    let is_unsafe = f.modifiers.iter().any(|m| m.kind == TokenKind::Unsafe);
                    let ret_type = self.resolve_type(&f.return_type);
                    self.env.set(f.name.clone(), Type::Function { is_unsafe, return_type: Box::new(ret_type) });
                },
                Declaration::Enum(e) => {
                    self.decls.insert(e.name.clone(), decl.clone());
                    self.env.set(e.name.clone(), Type::Enum { name: e.name.clone() });
                },
                Declaration::Struct(s) => {
                    self.decls.insert(s.name.clone(), decl.clone());
                    self.env.set(s.name.clone(), Type::Struct { name: s.name.clone() });
                    for (f_name, f_type) in &s.fields {
                        self.env.set(format!("{}.{}", s.name, f_name), self.resolve_type(f_type));
                    }
                },
                Declaration::Impl(i) => {
                    let mut full_target = i.target_name.clone();
                    if !i.generic_params.is_empty() { full_target = format!("{}<{}>", i.target_name, i.generic_params.join(", ")); }
                    let base_target = if i.target_name.contains('<') { i.target_name.split('<').next().unwrap_or(&i.target_name) } else { &i.target_name };
                    for f in &i.functions {
                        let name = format!("{}::{}", base_target, f.name);
                        self.decls.insert(name.clone(), Declaration::Function(f.clone()));
                        let is_unsafe = f.modifiers.iter().any(|m| m.kind == TokenKind::Unsafe);
                        let mut ret_name = f.return_type.clone();
                        if ret_name == "Self" { ret_name = full_target.clone(); }
                        let ret_type = self.resolve_type(&ret_name);
                        self.env.set(name, Type::Function { is_unsafe, return_type: Box::new(ret_type) });
                    }
                },
                _ => {}
            }
        }

        for decl in &program.declarations {
            match decl {
                Declaration::Function(f) => {
                    if let Some(body) = &f.body {
                        let was_unsafe = self.in_unsafe_context;
                        if f.modifiers.iter().any(|m| m.kind == TokenKind::Unsafe) { self.in_unsafe_context = true; }
                        let enclosed = Environment::new_enclosed(self.env.clone());
                        let old_env = std::mem::replace(&mut self.env, enclosed);
                        for (p_name, p_type, _) in &f.params { self.env.set(p_name.clone(), self.resolve_type(p_type)); }
                        for stmt in body { self.check_statement(stmt)?; }
                        self.env = old_env;
                        self.in_unsafe_context = was_unsafe;
                    }
                },
                Declaration::Impl(i) => {
                    for f in &i.functions {
                        if let Some(body) = &f.body {
                            let was_unsafe = self.in_unsafe_context;
                            if f.modifiers.iter().any(|m| m.kind == TokenKind::Unsafe) { self.in_unsafe_context = true; }
                            let enclosed = Environment::new_enclosed(self.env.clone());
                            let old_env = std::mem::replace(&mut self.env, enclosed);
                            for (p_name, p_type, _) in &f.params {
                                let mut pt = p_type.clone();
                                if pt == "Self" { pt = i.target_name.clone(); }
                                self.env.set(p_name.clone(), self.resolve_type(&pt));
                            }
                            for stmt in body { self.check_statement(stmt)?; }
                            self.env = old_env;
                            self.in_unsafe_context = was_unsafe;
                        }
                    }
                },
                _ => {}
            }
        }
        Ok(())
    }

    fn check_statement(&mut self, stmt: &Statement) -> Result<Type, CompileError> {
        match stmt {
            Statement::Let { name, value, .. } => {
                let val_type = self.check_expression(value)?;
                self.env.set(name.clone(), val_type);
                Ok(Type::Unit)
            },
            Statement::Assignment { target, value, .. } => {
                self.check_expression(target)?;
                self.check_expression(value)?;
                Ok(Type::Unit)
            },
            Statement::Return { value, .. } => { self.check_expression(value)?; Ok(Type::Unit) },
            Statement::ExpressionStmt(expr, _) => self.check_expression(expr),
            Statement::If { condition, then_branch, else_branch, .. } => {
                self.check_expression(condition)?;
                for s in then_branch { self.check_statement(s)?; }
                if let Some(eb) = else_branch { for s in eb { self.check_statement(s)?; } }
                Ok(Type::Unit)
            },
            Statement::While { condition, body, .. } => {
                self.check_expression(condition)?;
                for s in body { self.check_statement(s)?; }
                Ok(Type::Unit)
            },
            Statement::UnsafeBlock(body, _) => {
                let was = self.in_unsafe_context; self.in_unsafe_context = true;
                for s in body { self.check_statement(s)?; }
                self.in_unsafe_context = was; Ok(Type::Unit)
            },
            Statement::Match { condition, arms, .. } => {
                let cond_type = self.check_expression(condition)?;
                let cond_name = match &cond_type {
                    Type::Enum { name } => name.clone(),
                    Type::GenericInstance(name, _) => name.clone(),
                    Type::Integer => "i64".to_string(),
                    Type::String => "String".to_string(),
                    Type::Struct { name } => name.clone(),
                    _ => "unknown".to_string(),
                };
                for arm in arms {
                    let old_env = self.env.clone();
                    
                    // Get all patterns
                    let all_patterns: Vec<String> = if arm.patterns.is_empty() {
                        vec![arm.pattern.clone()]
                    } else {
                        arm.patterns.clone()
                    };
                    
                    // Check if this is a binding variable pattern (lowercase identifier that should bind)
                    let _is_binding_pattern = arm.params.len() > 0 && all_patterns.iter().all(|p| {
                        !p.parse::<i64>().is_ok() && !p.starts_with('"')
                    });
                    
                    if !arm.params.is_empty() {
                        self.env = Environment::new_enclosed(old_env.clone());
                        let mut payload = Type::Integer;
                        
                        // Check patterns for enum type
                        if let Some(Declaration::Enum(e)) = self.decls.get(&cond_name) {
                            for pat in &all_patterns {
                                for v in &e.variants {
                                    if pat == &v.name || pat.ends_with(&format!(".{}", v.name)) || pat.ends_with(&format!("::{}", v.name)) {
                                        if !v.data_types.is_empty() { payload = self.resolve_type(&v.data_types[0]); }
                                        break;
                                    }
                                }
                            }
                        } else if cond_name == "i64" {
                            payload = Type::Integer;
                        } else if cond_name == "String" {
                            payload = Type::String;
                        } else if let Some(Declaration::Struct(s)) = self.decls.get(&cond_name) {
                            payload = Type::Struct { name: cond_name.clone() };
                            // Add struct fields to environment
                            for (field_name, field_type_str) in &s.fields {
                                let field_type = self.resolve_type(field_type_str);
                                self.env.set(format!("{}.{}", arm.params[0], field_name), field_type);
                            }
                        }
                        
                        self.env.set(arm.params[0].clone(), payload);
                    }
                    
                    // Evaluate guard if present
                    if let Some(guard_expr) = &arm.guard {
                        self.check_expression(guard_expr)?;
                    }
                    
                    for s in &arm.body { self.check_statement(s)?; }
                    self.env = old_env;
                }
                Ok(Type::Unit)
            },
            _ => Ok(Type::Unit),
        }
    }

    fn check_expression(&mut self, expr: &Expression) -> Result<Type, CompileError> {
        match expr {
            Expression::Integer(..) => Ok(Type::Integer),
            Expression::Float(..) => Ok(Type::Float),
            Expression::Boolean(..) => Ok(Type::Boolean),
            Expression::String(..) => Ok(Type::String),
            Expression::Duration(..) => Ok(Type::Duration),
            Expression::Date(..) => Ok(Type::Date),
            Expression::TypeRef { name, generic_args, .. } => {
                let mut ga = Vec::new();
                for arg in generic_args { ga.push(self.env.get(arg).unwrap_or(Type::Unknown)); }
                Ok(Type::GenericInstance(name.clone(), ga))
            },
            Expression::Identifier(name, _) => {
                if let Some(t) = self.env.get(name) { return Ok(t); }
                if let Some((var, field)) = name.split_once('.')
                    && let Ok(rt) = self.check_expression(&Expression::Identifier(var.to_string(), Span::zero())) {
                        let tn = match rt { Type::GenericInstance(n, _) | Type::Struct { name: n } => n, _ => "".to_string() };
                        if !tn.is_empty() {
                            let full = self.resolve_fuzzy_name(&self.decls, &tn).unwrap_or(tn);
                            if let Some(t) = self.env.get(&format!("{}.{}", full, field)) { return Ok(t); }
                        }
                    }
                Ok(Type::Unknown)
            },
            Expression::Infix { left, operator, right, .. } => {
                let t1 = self.check_expression(left)?;
                let t2 = self.check_expression(right)?;
                self.check_compatibility(t1, t2, operator)
            },
            Expression::Call { function, arguments, .. } => {
                let span = expr.span();
                let call_expr = Expression::Call { function: function.clone(), generic_args: vec![], arguments: arguments.clone(), span };
                if let Some((receiver_name, method_name)) = function.rsplit_once('.') {
                    let receiver_expr = Expression::Identifier(receiver_name.to_string(), Span::zero());
                    if let Ok(rt) = self.check_expression(&receiver_expr) {
                        let mut is_ptr = false;
                        if let Type::Pointer(_) = rt { is_ptr = true; }
                        if is_ptr && method_name == "offset" {
                            for arg in arguments { self.check_expression(arg)?; }
                            return Ok(rt.clone());
                        }
                        if rt != Type::Unknown {
                            let tn = match rt {
                                Type::GenericInstance(ref n, _) | Type::Struct { name: ref n } | Type::Enum { name: ref n } => n.clone(),
                                Type::Integer => "i64".to_string(),
                                Type::String => "String".to_string(),
                                _ => "".to_string()
                            };
                            if !tn.is_empty() {
                                let full = self.resolve_fuzzy_name(&self.decls, &tn).unwrap_or(tn);
                                // Try both :: and . formats for method lookup
                                let cand_colon = format!("{}::{}", full, method_name);
                                let cand_dot = format!("{}.{}", full, method_name);
                                let ft = self.env.get(&cand_colon).or_else(|| self.env.get(&cand_dot));
                                if let Some(Type::Function { is_unsafe, ref return_type }) = ft {
                                    if is_unsafe && !self.in_unsafe_context {
                                        return Err(self.err(format!("unsafe method call '{}'", method_name), &call_expr));
                                    }
                                    for arg in arguments { self.check_expression(arg)?; }
                                    return Ok(*return_type.clone());
                                }
                            }
                        }
                    }
                }
                let ft = if let Some(t) = self.env.get(function) { t }
                         else if self.in_unsafe_context && function.starts_with("aion_") { Type::Function { is_unsafe: true, return_type: Box::new(Type::Unknown) } }
                         else { return Err(self.err(format!("function '{}' not defined", function), &call_expr)); };
                if let Type::Function { is_unsafe, ref return_type } = ft {
                    if is_unsafe && !self.in_unsafe_context {
                        return Err(self.err(format!("call to unsafe function '{}' requires unsafe block", function), &call_expr));
                    }
                    for arg in arguments { self.check_expression(arg)?; }
                    Ok(*return_type.clone())
                } else { Err(self.err(format!("'{}' is not a function", function), &call_expr)) }
            },
            Expression::MemberAccess { receiver, member, .. } => {
                let rt = self.check_expression(receiver)?;
                let span = receiver.span();
                let tn = match rt { Type::GenericInstance(ref n, _) | Type::Struct { name: ref n } => n.clone(), _ => return Err(CompileError::new(format!("member access on {:?}", rt), span.line, span.col).with_snippet(&self.source)) };
                let full = self.resolve_fuzzy_name(&self.decls, &tn).unwrap_or(tn);
                self.env.get(&format!("{}.{}", full, member))
                    .ok_or_else(|| CompileError::new(format!("field '{}' not found on struct '{}'", member, full), span.line, span.col).with_snippet(&self.source))
            },
            Expression::MethodCall { receiver, method, generic_args: _, arguments, .. } => {
                let method_expr = Expression::MethodCall { receiver: receiver.clone(), method: method.clone(), generic_args: vec![], arguments: arguments.clone(), span: expr.span() };
                let rt = self.check_expression(receiver)?;
                
                // Special case for Pointer.offset()
                if method == "offset"
                    && let Type::Pointer(_) = rt {
                        // Check argument is integer
                        if !arguments.is_empty() {
                            let arg_type = self.check_expression(&arguments[0])?;
                            if arg_type != Type::Integer { return Err(self.err("offset argument must be an integer", &method_expr)); }
                        }
                        return Ok(rt); // offset returns same pointer type
                    }

                let tn = match rt { 
                    Type::GenericInstance(ref n, _) | Type::Struct { name: ref n } | Type::Enum { name: ref n } => n.clone(), 
                    Type::Integer => "i64".to_string(),
                    Type::String => "String".to_string(),
                    _ => return Err(self.err(format!("method call on {:?}", rt), &method_expr)) 
                };

                let full = self.resolve_fuzzy_name(&self.decls, &tn).unwrap_or(tn);
                let cand = format!("{}::{}", full, method);
                let ft = self.env.get(&cand).ok_or_else(|| self.err(format!("method '{}' not found on '{}'", method, full), &method_expr))?;
                if let Type::Function { is_unsafe, ref return_type } = ft {
                    if is_unsafe && !self.in_unsafe_context {
                        return Err(self.err(format!("unsafe method call '{}'", method), &method_expr));
                    }
                    for arg in arguments { self.check_expression(arg)?; }
                    Ok(*return_type.clone())
                } else { Err(self.err(format!("'{}' is not a function", cand), &method_expr)) }
            },
            Expression::Cast { target, .. } => Ok(self.resolve_type(target)),
            Expression::StructInst { name, .. } => {
                let full = self.resolve_fuzzy_name(&self.decls, name).unwrap_or(name.clone());
                Ok(Type::Struct { name: full })
            },
            Expression::EnumInst { name, .. } => {
                let full = self.resolve_fuzzy_name(&self.decls, name).unwrap_or(name.clone());
                Ok(Type::Enum { name: full })
            },
            Expression::Deref { expr, .. } => {
                let rt = self.check_expression(expr)?;
                if let Type::Pointer(t) = rt { Ok(*t) } else { Ok(Type::Integer) }
            },
            Expression::Intrinsic { name, arguments, .. } => {
                let mut actual_name = name.clone();
                let mut args = arguments.as_slice();
                if name == "intrinsic" && !arguments.is_empty()
                    && let Expression::String(s, _) = &arguments[0] {
                        actual_name = s.clone();
                        args = &arguments[1..];
                    }
                for arg in args { self.check_expression(arg)?; }
                if actual_name == "str_len" || actual_name == "fs_exists" || actual_name == "fs_write" || actual_name == "fs_append" { Ok(Type::Integer) }
                else if actual_name == "str_concat" || actual_name == "fs_read_to_string" || actual_name == "int_to_str" || actual_name == "float_to_str" || actual_name == "char_to_str" || actual_name == "str_substr" { Ok(Type::String) }
                else if actual_name == "str_ptr" { Ok(Type::Pointer(Box::new(Type::Integer))) }
                else if actual_name == "mem_is_null" { Ok(Type::Boolean) }
                else if actual_name.starts_with("ai_tensor_") { Ok(Type::Struct { name: "std.ai.tensor.Tensor".to_string() }) }
                else { Ok(Type::Integer) }
            },
            Expression::If { condition, then_branch, else_branch, .. } => {
                self.check_expression(condition)?;
                let mut lt = Type::Unit;
                for s in then_branch { lt = self.check_statement(s)?; }
                if let Some(eb) = else_branch { for s in eb { self.check_statement(s)?; } }
                Ok(lt)
            },
            Expression::Block { statements, is_unsafe, .. } => {
                let was = self.in_unsafe_context;
                if *is_unsafe { self.in_unsafe_context = true; }
                let mut lt = Type::Unit;
                for s in statements { lt = self.check_statement(s)?; }
                if *is_unsafe { self.in_unsafe_context = was; }
                Ok(lt)
            },
            _ => Err(CompileError::Internal(format!("Unsupported expression {:?}", expr))),
        }
    }

    fn check_compatibility(&self, t1: Type, t2: Type, op: &Token) -> Result<Type, CompileError> {
        match t1 {
            Type::Integer => {
                if t2 == Type::Integer {
                    if matches!(op.kind, TokenKind::Plus | TokenKind::Minus | TokenKind::Star | TokenKind::Slash | TokenKind::Percent | TokenKind::And | TokenKind::Or | TokenKind::Caret) { 
                        return Ok(Type::Integer); 
                    }
                    if matches!(op.kind, TokenKind::EqEq | TokenKind::NotEq | TokenKind::Lt | TokenKind::Gt | TokenKind::LtEq | TokenKind::GtEq) { 
                        return Ok(Type::Boolean); 
                    }
                    return Ok(Type::Integer);
                }
            },
            Type::Float => {
                if t2 == Type::Float {
                    if matches!(op.kind, TokenKind::Plus | TokenKind::Minus | TokenKind::Star | TokenKind::Slash) { return Ok(Type::Float); }
                    if matches!(op.kind, TokenKind::EqEq | TokenKind::NotEq | TokenKind::Lt | TokenKind::Gt | TokenKind::LtEq | TokenKind::GtEq) { return Ok(Type::Boolean); }
                    return Ok(Type::Float);
                }
            },
            Type::Boolean => {
                if t2 == Type::Boolean {
                    if matches!(op.kind, TokenKind::And | TokenKind::Or | TokenKind::EqEq | TokenKind::NotEq) { return Ok(Type::Boolean); }
                    return Ok(Type::Boolean);
                }
            },
            Type::String => {
                if (t2 == Type::String || t2 == Type::Integer)
                    && op.kind == TokenKind::Plus { return Ok(Type::String); }
                if t2 == Type::String
                    && matches!(op.kind, TokenKind::EqEq | TokenKind::NotEq) { return Ok(Type::Boolean); }
            },
            Type::Placeholder(_) => { return Ok(t1.clone()); },
            Type::Date => {
                if t2 == Type::Duration && op.kind == TokenKind::Plus { return Ok(Type::Date); }
            },
            _ => {
                if t1 == t2 && matches!(op.kind, TokenKind::EqEq | TokenKind::NotEq) { return Ok(Type::Boolean); }
            }
        }
        if let Type::Placeholder(_) = t2 { return Ok(t1.clone()); }
        Err(CompileError::InvalidOperator {
            op: format!("{:?}", op.kind),
            left: format!("{:?}", t1),
            right: format!("{:?}", t2),
            line: op.line,
            col: op.col,
            snippet: None,
        })
    }

    fn resolve_fuzzy_name<T>(&self, map: &HashMap<String, T>, name: &str) -> Option<String> {
        if map.contains_key(name) { return Some(name.to_string()); }
        for key in map.keys() {
            if key.ends_with(name) && (key.len() == name.len() || key.as_bytes()[key.len() - name.len() - 1] == b'.') {
                return Some(key.clone());
            }
        }
        None
    }
}
