; ModuleID = 'aion_module'
source_filename = "aion_module"

declare i32 @printf(ptr, ...)

declare void @aion_spawn(ptr)

define i64 @main() {
entry:
  %status = alloca i64, align 8
  store i64 200, ptr %status, align 4
  %logger = alloca i64, align 8
  store i64 0, ptr %logger, align 4
  %status1 = load i64, ptr %status, align 4
  ret i64 %status1
}
