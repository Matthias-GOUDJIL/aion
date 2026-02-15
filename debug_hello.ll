; ModuleID = 'aion_module'
source_filename = "aion_module"

@aion_str = private unnamed_addr constant [28 x i8] c"Hello from Test Harness!\\n\0A\00", align 1

declare i32 @printf(ptr, ...)

declare void @aion_spawn(ptr)

declare double @pow(double, double)

define i64 @main() {
entry:
  %printftmp = call i32 (ptr, ...) @printf(ptr @aion_str)
  ret i64 0
}
