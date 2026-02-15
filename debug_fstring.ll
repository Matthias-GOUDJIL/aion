; ModuleID = 'aion_module'
source_filename = "aion_module"

@aion_str = private unnamed_addr constant [7 x i8] c"World\0A\00", align 1
@aion_str.1 = private unnamed_addr constant [30 x i8] c"Hello {name}, you are {age}!\0A\00", align 1

declare i32 @printf(ptr, ...)

declare void @aion_spawn(ptr)

declare double @pow(double, double)

define i64 @main() {
entry:
  %name = alloca ptr, align 8
  store ptr @aion_str, ptr %name, align 8
  %age = alloca i64, align 8
  store i64 30, ptr %age, align 4
  %msg = alloca ptr, align 8
  store ptr @aion_str.1, ptr %msg, align 8
  %msg1 = load i64, ptr %msg, align 4
  %printftmp = call i32 (ptr, ...) @printf(i64 %msg1)
  %age2 = load i64, ptr %age, align 4
  ret i64 %age2
}
