use crate::ast::{Declaration, Function};
use crate::codegen::compiler::Compiler;

impl<'ctx> Compiler<'ctx> {
    /// Register the built-in intrinsic functions backed by the C runtime
    /// (`src/runtime.c`) or libc. Each entry is exposed in Aion as a callable
    /// function whose name is the Aion-facing name and whose `attribute` value
    /// is the C function the codegen resolves `@intrinsic(...)` calls to.
    ///
    /// Phase 1 of the `compiler.rs` split (#113): moved verbatim from
    /// `src/codegen/compiler.rs` to establish the submodule pattern and shrink
    /// the monolithic codegen file. Behaviour-preserving code motion — the
    /// 96 integration snapshots are byte-identical before/after.
    /// Register the built-in intrinsic functions backed by the C runtime
    /// (`src/runtime.c`) or libc. Derived from the single `BUILTINS` table
    /// in `src/builtins.rs` — the checker's env, these codegen decls and
    /// the extern declarations all consume the same data (#177).
    pub(in crate::codegen) fn register_builtins(&mut self) {
        for b in crate::builtins::BUILTINS {
            let params: Vec<(String, String, Option<Box<crate::ast::Expression>>)> = b
                .params_aion
                .iter()
                .map(|(n, t)| (n.to_string(), t.to_string(), None))
                .collect();
            let d = Declaration::Function(Function {
                name: b.aion_name.to_string(),
                generic_params: vec![],
                params,
                return_type: b.ret_aion.to_string(),
                body: None,
                modifiers: vec![],
                attributes: vec![("intrinsic".to_string(), b.llvm_name.to_string())],
                doc_comment: None,
            });
            self.decls.insert(b.aion_name.to_string(), d);
        }
    }

    /// Substitute generic placeholders by exact-token matching inside a type
    /// string. A token containing `.` (a qualified name) is never replaced:
    /// generic params are always bare identifiers, never dotted paths.
    ///
    /// Phase 1 of the `compiler.rs` split (#113): moved verbatim.
    pub(in crate::codegen) fn substitute_type_string(
        s: &str,
        params: &[String],
        args: &[String],
    ) -> String {
        if params.is_empty() || s.is_empty() {
            return s.to_string();
        }
        let mut out = String::with_capacity(s.len());
        let mut tok = String::new();
        let flush = |tok: &mut String, out: &mut String| {
            if !tok.is_empty() {
                let mut replaced = false;
                for (i, p) in params.iter().enumerate() {
                    if i < args.len() && *tok == *p {
                        out.push_str(&args[i]);
                        replaced = true;
                        break;
                    }
                }
                if !replaced {
                    out.push_str(tok);
                }
                tok.clear();
            }
        };
        for ch in s.chars() {
            if ch.is_alphanumeric() || ch == '_' || ch == '.' {
                tok.push(ch);
            } else {
                flush(&mut tok, &mut out);
                out.push(ch);
            }
        }
        flush(&mut tok, &mut out);
        out
    }

    /// `substitute_type_string` wrapper kept as an associated function so the
    /// existing `Self::substitute_generic_params(...)` call sites compile
    /// unchanged after the split. Phase 1 of #113.
    pub(in crate::codegen) fn substitute_generic_params(
        res_type: &str,
        params: &[String],
        args: &[String],
    ) -> String {
        Self::substitute_type_string(res_type, params, args)
    }
}
