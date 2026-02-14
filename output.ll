; ModuleID = 'aion_module'
source_filename = "aion_module"

@aion_str = private unnamed_addr constant [19 x i8] c"Main Thread Start\0A\00", align 1
@aion_str.1 = private unnamed_addr constant [27 x i8] c"Spark Thread: I am alive!\0A\00", align 1
@aion_str.2 = private unnamed_addr constant [17 x i8] c"Main Thread End\0A\00", align 1

declare i32 @printf(ptr, ...)

declare void @aion_spawn(ptr)

define i64 @main() {
entry:
  %printftmp = call i32 (ptr, ...) @printf(ptr @aion_str)
  call void @aion_spawn(ptr @aion_spark_handler)
  %printftmp1 = call i32 (ptr, ...) @printf(ptr @aion_str.2)
  ret i64 0
}

define void @aion_spark_handler() {
entry:
  %printftmp = call i32 (ptr, ...) @printf(ptr @aion_str.1)
  ret void
}
