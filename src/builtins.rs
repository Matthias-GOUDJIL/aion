//! Single source of truth for the builtin functions backed by the C runtime
//! (`src/runtime.c`) or libc. #177.
//!
//! Before this module, three hand-written tables had to agree manually —
//! the checker's `register_builtins` (env types), the codegen's
//! `register_builtins` (decls + intrinsic attributes) and the extern
//! declarations in `Compiler::compile`. They diverged (e.g. `fs_write`
//! returned `i32` in codegen but `i64` in the checker, `ai.tensor_matmul`
//! was registered but never declared). All three consumers now derive
//! from `BUILTINS` below.

/// LLVM parameter shape for the runtime symbol declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlvmParam {
    Ptr,
    I64,
    F64,
    I32,
}

impl LlvmParam {
    /// The Aion-facing parameter type-name string for this LLVM shape.
    pub fn aion_type(&self) -> &'static str {
        match self {
            LlvmParam::Ptr => "String",
            LlvmParam::I64 | LlvmParam::I32 => "i64",
            LlvmParam::F64 => "f64",
        }
    }
}

/// One builtin: its Aion-facing name, the runtime symbol it lowers to,
/// the Aion return type-name, Aion parameter (name, type-name) list (the
/// first parameter named `self` marks a method-style builtin), the unsafe
/// flag, and the LLVM shapes used to declare the runtime symbol.
#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub aion_name: &'static str,
    pub llvm_name: &'static str,
    pub ret_aion: &'static str,
    /// (param name, Aion type-name). First named `self` => method-style.
    pub params_aion: &'static [(&'static str, &'static str)],
    pub is_unsafe: bool,
    pub llvm_params: &'static [LlvmParam],
}

pub static BUILTINS: &[Builtin] = &[
    Builtin {
        aion_name: "io.println",
        llvm_name: "aion_io_println",
        ret_aion: "void",
        params_aion: &[("s", "String")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "io.print",
        llvm_name: "aion_io_print",
        ret_aion: "void",
        params_aion: &[("s", "String")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "io.read_line",
        llvm_name: "aion_io_read_line",
        ret_aion: "String",
        params_aion: &[],
        is_unsafe: false,
        llvm_params: &[],
    },
    Builtin {
        aion_name: "string.from_int",
        llvm_name: "aion_int_to_str",
        ret_aion: "String",
        params_aion: &[("n", "i64")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::I64],
    },
    Builtin {
        aion_name: "string.from_float",
        llvm_name: "aion_float_to_str",
        ret_aion: "String",
        params_aion: &[("f", "f64")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::F64],
    },
    Builtin {
        aion_name: "string.to_float",
        llvm_name: "aion_str_to_float",
        ret_aion: "f64",
        params_aion: &[("s", "String")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "fs_read_to_string",
        llvm_name: "aion_read_file",
        ret_aion: "String",
        params_aion: &[("p", "String")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "fs_write",
        llvm_name: "aion_write_file",
        ret_aion: "i64",
        params_aion: &[("p", "String"), ("c", "String")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr, LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "fs_exists",
        llvm_name: "aion_fs_exists",
        ret_aion: "i64",
        params_aion: &[("p", "String")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "fs_append",
        llvm_name: "aion_append_file",
        ret_aion: "i64",
        params_aion: &[("p", "String"), ("c", "String")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr, LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "aion_read_file",
        llvm_name: "aion_read_file",
        ret_aion: "String",
        params_aion: &[("p", "String")],
        is_unsafe: true,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "aion_write_file",
        llvm_name: "aion_write_file",
        ret_aion: "i64",
        params_aion: &[("p", "String"), ("c", "String")],
        is_unsafe: true,
        llvm_params: &[LlvmParam::Ptr, LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "aion_getenv",
        llvm_name: "aion_getenv",
        ret_aion: "String",
        params_aion: &[("k", "String")],
        is_unsafe: true,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "aion_get_argc",
        llvm_name: "aion_get_argc",
        ret_aion: "i64",
        params_aion: &[],
        is_unsafe: true,
        llvm_params: &[],
    },
    Builtin {
        aion_name: "aion_get_argv_index",
        llvm_name: "aion_get_argv_index",
        ret_aion: "String",
        params_aion: &[("i", "i64")],
        is_unsafe: true,
        llvm_params: &[LlvmParam::I64],
    },
    Builtin {
        aion_name: "aion_exit",
        llvm_name: "exit",
        ret_aion: "void",
        params_aion: &[("c", "i64")],
        is_unsafe: true,
        llvm_params: &[LlvmParam::I32],
    },
    Builtin {
        aion_name: "exit",
        llvm_name: "exit",
        ret_aion: "void",
        params_aion: &[("c", "i64")],
        is_unsafe: true,
        llvm_params: &[LlvmParam::I32],
    },
    Builtin {
        aion_name: "aion_malloc",
        llvm_name: "aion_malloc",
        ret_aion: "ptr",
        params_aion: &[("s", "i64")],
        is_unsafe: true,
        llvm_params: &[LlvmParam::I64],
    },
    Builtin {
        aion_name: "aion_realloc",
        llvm_name: "aion_realloc",
        ret_aion: "ptr",
        params_aion: &[("p", "ptr"), ("s", "i64")],
        is_unsafe: true,
        llvm_params: &[LlvmParam::Ptr, LlvmParam::I64],
    },
    Builtin {
        aion_name: "aion_free",
        llvm_name: "aion_free",
        ret_aion: "void",
        params_aion: &[("p", "ptr")],
        is_unsafe: true,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "aion_str_at",
        llvm_name: "aion_str_at",
        ret_aion: "i64",
        params_aion: &[("s", "String"), ("i", "i64")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr, LlvmParam::I64],
    },
    Builtin {
        aion_name: "aion_str_substr",
        llvm_name: "aion_str_substr",
        ret_aion: "String",
        params_aion: &[("s", "String"), ("i", "i64"), ("l", "i64")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr, LlvmParam::I64, LlvmParam::I64],
    },
    Builtin {
        aion_name: "aion_char_to_str",
        llvm_name: "aion_char_to_str",
        ret_aion: "String",
        params_aion: &[("c", "i64")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::I64],
    },
    Builtin {
        aion_name: "ai.tensor_zeros",
        llvm_name: "aion_ai_tensor_zeros",
        ret_aion: "ptr",
        params_aion: &[("shape", "ptr")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "ai.tensor_ones",
        llvm_name: "aion_ai_tensor_ones",
        ret_aion: "ptr",
        params_aion: &[("shape", "ptr")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "ai.tensor_rand",
        llvm_name: "aion_ai_tensor_rand",
        ret_aion: "ptr",
        params_aion: &[("shape", "ptr")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "ai.tensor_backward",
        llvm_name: "aion_ai_tensor_backward",
        ret_aion: "void",
        params_aion: &[("t", "ptr")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "ai.tensor_matmul",
        llvm_name: "aion_ai_tensor_matmul",
        ret_aion: "ptr",
        params_aion: &[("t1", "ptr"), ("t2", "ptr")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr, LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "ai.tensor_add",
        llvm_name: "aion_ai_tensor_add",
        ret_aion: "ptr",
        params_aion: &[("t1", "ptr"), ("t2", "ptr")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr, LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "ai.tensor_move",
        llvm_name: "aion_ai_tensor_move",
        ret_aion: "ptr",
        params_aion: &[("t", "ptr"), ("dev", "String")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr, LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "i64.abs",
        llvm_name: "aion_i64_abs",
        ret_aion: "i64",
        params_aion: &[("self", "i64")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::I64],
    },
    Builtin {
        aion_name: "i64.max",
        llvm_name: "aion_i64_max",
        ret_aion: "i64",
        params_aion: &[("self", "i64"), ("other", "i64")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::I64, LlvmParam::I64],
    },
    Builtin {
        aion_name: "i64.min",
        llvm_name: "aion_i64_min",
        ret_aion: "i64",
        params_aion: &[("self", "i64"), ("other", "i64")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::I64, LlvmParam::I64],
    },
    Builtin {
        aion_name: "string.len",
        llvm_name: "aion_string_len",
        ret_aion: "i64",
        params_aion: &[("self", "String")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr],
    },
    Builtin {
        aion_name: "String.len",
        llvm_name: "aion_string_len",
        ret_aion: "i64",
        params_aion: &[("self", "String")],
        is_unsafe: false,
        llvm_params: &[LlvmParam::Ptr],
    },
];
