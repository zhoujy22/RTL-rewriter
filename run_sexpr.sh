#!/bin/bash
# 设置路径变量
DIR="ITC99-RTL-Verilog"
SEXPR_OUTPUT="ITC99-RTL-S_expression"
for file in "$DIR"/*; do
    if [ -f "$file" ]; then
        echo " 正在使用 stagira 生成 S-expression..."
        ./stagira -toegg "$file" > "$SEXPR_OUTPUT"/$(basename "$file" .v).sexpr
    fi
done
