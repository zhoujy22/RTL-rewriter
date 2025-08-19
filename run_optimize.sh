#!/bin/bash

# 设置路径变量
TEST_DIR="./test"
INPUT_VERILOG="$TEST_DIR/test_case.v"
SEXPR_OUTPUT="./e-rewriter/input.sexpr"
SEXPR_OPTIMIZED="$TEST_DIR/optimized.sexpr"
REWRITER_BIN="./target/debug/e-rewriter"

if [ ! -f "$INPUT_VERILOG" ]; then
    echo " Verilog 文件未找到: $INPUT_VERILOG"
    exit 1
fi

# Step 1: 使用 stagira 生成 S-expression
echo " 正在使用 stagira 生成 S-expression..."
./stagira -toegg "$INPUT_VERILOG" > "$SEXPR_OUTPUT"

# 检查生成是否成功
if [ $? -ne 0 ]; then
    echo " stagira 生成失败"
    exit 1
fi
echo " 已生成前缀表达式: $SEXPR_OUTPUT"

# Step 2: 调用 e-rewriter 优化表达式
echo " 正在调用 e-rewriter 优化表达式..."
cd e-rewriter || { echo " 进入 e-rewriter 目录失败"; exit 1; }
"$REWRITER_BIN"

# Step 3: 重新生成verilog
echo " 正在重新生成 Verilog 文件..."
cd ..
./stagira -rsexp "$SEXPR_OPTIMIZED" -psexp2ast > "$TEST_DIR/optimized.v"
# 可选：保存输出到文件
# "$REWRITER_BIN" > e-rewriter/output.sexpr

# 完成
echo " 优化流程完成"