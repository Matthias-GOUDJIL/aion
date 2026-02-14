; ModuleID = 'aion_module'
source_filename = "aion_module"

@aion_str = private unnamed_addr constant [14 x i8] c"Aion is Live\0A\00", align 1

declare i32 @printf(ptr, ...)

define i64 @main() {
entry:
  %printftmp = call i32 (ptr, ...) @printf(ptr @aion_str)
  ret i64 42
}
