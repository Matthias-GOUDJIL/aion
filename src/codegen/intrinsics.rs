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
    pub(in crate::codegen) fn register_builtins(&mut self) {
        let builtins = vec![
            ("io.println", "aion_io_println", "void", false),
            ("io.print", "aion_io_print", "void", false),
            ("io.read_line", "aion_io_read_line", "String", false),
            ("string.from_int", "aion_int_to_str", "String", false),
            ("string.from_float", "aion_float_to_str", "String", false),
            ("string.to_float", "aion_str_to_float", "f64", false),
            ("fs_read_to_string", "aion_read_file", "String", false),
            ("fs_write", "aion_write_file", "i32", false),
            ("fs_exists", "aion_fs_exists", "i64", false),
            ("fs_append", "aion_append_file", "i32", false),
            ("aion_getenv", "aion_getenv", "String", false),
            ("aion_get_argc", "aion_get_argc", "i64", false),
            (
                "aion_get_argv_index",
                "aion_get_argv_index",
                "String",
                false,
            ),
            ("aion_exit", "exit", "void", false),
            ("exit", "exit", "void", false),
            ("aion_malloc", "aion_malloc", "ptr", false),
            ("aion_realloc", "aion_realloc", "ptr", false),
            ("aion_free", "aion_free", "void", false),
            ("aion_str_at", "aion_str_at", "i64", false),
            ("aion_str_substr", "aion_str_substr", "String", false),
            ("aion_char_to_str", "aion_char_to_str", "String", false),
            ("ai.tensor_zeros", "aion_ai_tensor_zeros", "ptr", false),
            ("ai.tensor_ones", "aion_ai_tensor_ones", "ptr", false),
            ("ai.tensor_rand", "aion_ai_tensor_rand", "ptr", false),
            (
                "ai.tensor_backward",
                "aion_ai_tensor_backward",
                "void",
                false,
            ),
            ("ai.tensor_matmul", "aion_ai_tensor_matmul", "ptr", false),
            ("ai.tensor_add", "aion_ai_tensor_add", "ptr", false),
            ("ai.tensor_move", "aion_ai_tensor_move", "ptr", false),
            ("i64.abs", "aion_i64_abs", "i64", true),
            ("i64.max", "aion_i64_max", "i64", true),
            ("i64.min", "aion_i64_min", "i64", true),
            ("string.len", "aion_string_len", "i64", true),
            ("String.len", "aion_string_len", "i64", true),
        ];
        for (an, ln, rt, is_method) in builtins {
            let params = if is_method {
                vec![("self".to_string(), "i64".to_string(), None)]
            } else {
                vec![]
            };
            let d = Declaration::Function(Function {
                name: an.to_string(),
                generic_params: vec![],
                params,
                return_type: rt.to_string(),
                body: None,
                modifiers: vec![],
                attributes: vec![("intrinsic".to_string(), ln.to_string())],
                doc_comment: None,
            });
            self.decls.insert(an.to_string(), d);
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
