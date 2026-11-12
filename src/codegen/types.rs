use crate::analysis::types::Type;
use crate::codegen::compiler::Compiler;
use crate::error::CompileError;
use inkwell::AddressSpace;
use inkwell::types::{BasicTypeEnum, StructType};

impl<'ctx> Compiler<'ctx> {
    /// Lower an Aion type-name string (e.g. "i64", "[i64; 3]", "Vector<String>")
    /// to its LLVM `BasicTypeEnum` via the analysis `Type::parse` path. Phase 2
    /// of the `compiler.rs` split (#113).
    pub(in crate::codegen) fn aion_type_to_llvm(&self, tn: &str) -> BasicTypeEnum<'ctx> {
        self.type_to_llvm(&Type::parse(tn))
    }

    /// Extract the element LLVM type and count from a structural `Type::Array`.
    /// Replaces the former string-parsing `parse_array_type_name` (#108).
    pub(in crate::codegen) fn array_llvm_type(
        &self,
        ty: &Type,
    ) -> Option<(BasicTypeEnum<'ctx>, u64)> {
        if let Type::Array(elem, n) = ty {
            Some((self.type_to_llvm(elem), *n))
        } else {
            None
        }
    }

    /// Ensure an anonymous tuple struct type is registered for a structural
    /// `Type::Tuple` (`#53`, `#108`). Operates directly on the element `Type`
    /// slice without string splitting.
    pub(in crate::codegen) fn ensure_tuple_type(
        &mut self,
        ty: &Type,
    ) -> Result<StructType<'ctx>, CompileError> {
        let key = ty.name().replace(" ", "");
        if let Some(&st) = self.struct_types.get(&key) {
            return Ok(st);
        }
        let elems = match ty {
            Type::Tuple(elems) => elems,
            _ => return Err(CompileError::internal(format!("not a tuple type: {}", key))),
        };
        let mut elem_tys = Vec::with_capacity(elems.len());
        let mut fm = std::collections::HashMap::new();
        for (i, elem) in elems.iter().enumerate() {
            elem_tys.push(self.type_to_llvm(elem));
            fm.insert(i.to_string(), i as u32);
        }
        let st = self.context.struct_type(&elem_tys, false);
        self.struct_types.insert(key.clone(), st);
        self.struct_fields.insert(key, fm);
        Ok(st)
    }

    /// Coerce an integer value to the target LLVM int type, choosing
    /// sign-extension / zero-extension / truncation based on the structural
    /// `Type` (#52, #108).
    pub(in crate::codegen) fn coerce_int_width(
        &self,
        v: inkwell::values::IntValue<'ctx>,
        target: inkwell::types::IntType<'ctx>,
        ty: &Type,
    ) -> Result<inkwell::values::IntValue<'ctx>, CompileError> {
        let from_bits = v.get_type().get_bit_width();
        let to_bits = target.get_bit_width();
        if from_bits == to_bits {
            return Ok(v);
        }
        let signed = match ty {
            Type::Integer { signed, .. } => *signed,
            _ => ty.name().starts_with('i'),
        };
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
    pub(in crate::codegen) fn type_to_llvm(&self, ty: &Type) -> BasicTypeEnum<'ctx> {
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
