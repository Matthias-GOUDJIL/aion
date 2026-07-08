use crate::codegen::compiler::Compiler;
use crate::error::CompileError;
use inkwell::AddressSpace;
use inkwell::types::{BasicTypeEnum, StructType};

impl<'ctx> Compiler<'ctx> {
    /// Lower an Aion type-name string (e.g. "i64", "[i64; 3]", "Vector<String>")
    /// to its LLVM `BasicTypeEnum` via the analysis `Type::parse` path. Phase 2
    /// of the `compiler.rs` split (#113): moved verbatim from
    /// `src/codegen/compiler.rs` — behaviour-preserving code motion.
    pub(in crate::codegen) fn aion_type_to_llvm(&self, tn: &str) -> BasicTypeEnum<'ctx> {
        self.type_to_llvm(&crate::analysis::types::Type::parse(tn))
    }

    /// Parse an array type-name string `[T; N]` into the element LLVM type
    /// and the count. Returns None if `tn` is not an array type-name. #54.
    /// Phase 2 of #113: moved verbatim.
    pub(in crate::codegen) fn parse_array_type_name(
        &self,
        tn: &str,
    ) -> Option<(BasicTypeEnum<'ctx>, u64)> {
        let clean = tn.replace(" ", "");
        let inner = clean.strip_prefix('[').and_then(|s| s.strip_suffix(']'))?;
        // Top-level ';' split.
        let mut depth = 0i32;
        let mut split_at = None;
        for (i, c) in inner.chars().enumerate() {
            match c {
                '[' | '(' | '<' => depth += 1,
                ']' | ')' | '>' => depth -= 1,
                ';' if depth == 0 => {
                    split_at = Some(i);
                    break;
                }
                _ => {}
            }
        }
        let i = split_at?;
        let elem_str = inner[..i].trim();
        let size_str = inner[i + 1..].trim();
        let n = size_str.parse::<u64>().ok()?;
        Some((self.aion_type_to_llvm(elem_str), n))
    }

    /// Ensure a tuple struct type is registered for `tn` (e.g. "(i64,String)").
    /// If already in `struct_types`, return it; otherwise build the anonymous
    /// struct from the element type names, register it + its field map, and
    /// return it. This handles forward references (a caller may need the
    /// tuple type before the defining function's body is compiled, since
    /// `self.decls` is a HashMap with non-deterministic order). #53.
    /// Phase 2 of #113: moved verbatim.
    pub(in crate::codegen) fn ensure_tuple_type(
        &mut self,
        tn: &str,
    ) -> Result<StructType<'ctx>, CompileError> {
        if let Some(&st) = self.struct_types.get(tn) {
            return Ok(st);
        }
        let clean = tn.replace(" ", "");
        let inner = clean
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| CompileError::internal(format!("not a tuple type: {}", tn)))?;
        // Depth-aware comma split so nested tuples `(i64,(String,bool))`
        // split into `["i64", "(String,bool)"]`. #53.
        let mut elem_tys = Vec::new();
        let mut fm = std::collections::HashMap::new();
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
        for (i, part) in parts.iter().enumerate() {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            elem_tys.push(self.aion_type_to_llvm(p));
            fm.insert(i.to_string(), i as u32);
        }
        let st = self.context.struct_type(&elem_tys, false);
        self.struct_types.insert(tn.to_string(), st);
        self.struct_fields.insert(tn.to_string(), fm);
        Ok(st)
    }

    /// Coerce an integer value to the target LLVM int type, choosing
    /// sign-extension / zero-extension / truncation based on the Aion type
    /// name (`et_clean` like "i32"/"u8"). Used by `let x: i32 = <i64 literal>`
    /// and similar widening/narrowing sites. #52.
    /// Phase 2 of #113: moved verbatim.
    pub(in crate::codegen) fn coerce_int_width(
        &self,
        v: inkwell::values::IntValue<'ctx>,
        target: inkwell::types::IntType<'ctx>,
        et_clean: &str,
    ) -> Result<inkwell::values::IntValue<'ctx>, CompileError> {
        let from_bits = v.get_type().get_bit_width();
        let to_bits = target.get_bit_width();
        if from_bits == to_bits {
            return Ok(v);
        }
        // Signedness from the Aion type name (i* signed, u* unsigned).
        let signed = et_clean.starts_with('i');
        if to_bits > from_bits {
            if signed {
                Ok(self.builder.build_int_s_extend(v, target, "let_sext")?)
            } else {
                Ok(self.builder.build_int_z_extend(v, target, "let_zext")?)
            }
        } else {
            Ok(self.builder.build_int_truncate(v, target, "let_trunc")?)
        }
    }

    /// Lower an analysis `Type` to its LLVM `BasicTypeEnum`. Phase 2 of #113:
    /// moved verbatim. Owns the box-representation invariant documented in
    /// `docs/architecture.md` (structs/enums/tuples/arrays → opaque pointer).
    pub(in crate::codegen) fn type_to_llvm(
        &self,
        ty: &crate::analysis::types::Type,
    ) -> BasicTypeEnum<'ctx> {
        use crate::analysis::types::Type;
        match ty {
            // Emit the correctly-sized LLVM integer type so that `@sizeof`
            // and FFI layouts respect i8/i32/i64. #52.
            Type::Integer { bits, .. } => match bits {
                8 => self.context.i8_type().into(),
                16 => self.context.i16_type().into(),
                32 => self.context.i32_type().into(),
                _ => self.context.i64_type().into(),
            },
            Type::Boolean | Type::Date | Type::Duration | Type::Unit => {
                self.context.i64_type().into()
            }
            Type::Float => self.context.f64_type().into(),
            Type::String
            | Type::Pointer(_)
            | Type::Enum { .. }
            | Type::Struct { .. }
            | Type::GenericInstance(..)
            | Type::Tuple(..)
            | Type::Array(..) => self.context.ptr_type(AddressSpace::default()).into(),
            _ => self.context.ptr_type(AddressSpace::default()).into(),
        }
    }
}
