; ModuleID = 'aion_module'
source_filename = "aion_module"

declare i32 @printf(ptr, ...)

declare void @aion_spawn(ptr)

declare double @pow(double, double)

define i64 @double(i64 %x) {
entry:
  %x1 = alloca i64, align 8
  store i64 %x, ptr %x1, align 4
  %x2 = load i64, ptr %x1, align 4
  %multmp = mul i64 %x2, 2
  ret i64 %multmp
}

define i64 @add_five(i64 %x) {
entry:
  %x1 = alloca i64, align 8
  store i64 %x, ptr %x1, align 4
  %x2 = load i64, ptr %x1, align 4
  %addtmp = add i64 %x2, 5
  ret i64 %addtmp
}

define i64 @square(i64 %x) {
entry:
  %x1 = alloca i64, align 8
  store i64 %x, ptr %x1, align 4
  %x2 = load i64, ptr %x1, align 4
  %x3 = load i64, ptr %x1, align 4
  %multmp = mul i64 %x2, %x3
  ret i64 %multmp
}

define i64 @main() {
entry:
  %val = alloca i64, align 8
  store i64 2, ptr %val, align 4
  %val1 = load i64, ptr %val, align 4
  %calltmp = call i64 @double(i64 %val1)
  %calltmp2 = call i64 @add_five(i64 0)
  %calltmp3 = call i64 @square(i64 0)
  %result = alloca i64, align 8
  store i64 0, ptr %result, align 4
  %result4 = load i64, ptr %result, align 4
  ret i64 %result4
}
