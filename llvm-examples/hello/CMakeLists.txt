project(acca-llvm-examples-hello)

add_executable(hello
	hello.c
)

acca_apply_rules(hello)
