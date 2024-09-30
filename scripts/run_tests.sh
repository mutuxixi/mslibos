#!/bin/bash

declare -A test_groups=(
    [faasflow_test]="long_chain map_reduce pass_complex_args"
    [file_test]="simple_file"
    [wasm_test]="tinywasm_wordcount"
)

passed_count=0
declare -A results

# 获取脚本参数（feature）
feature_arg=""
if [ $# -gt 0 ]; then
    feature_arg="--features $1"
fi

for group in "${!test_groups[@]}"; do
    names=${test_groups[$group]}
    
    for name in $names; do
        output=$(RUST_LOG=info cargo run $feature_arg -- --files "isol_config/$name.json" 2>&1)
        echo "$output"
        if [ $? -eq 0 ]; then
            results[$name]="passed"
            ((passed_count++)) # 增加通过计数
        elif echo "$output" | grep -q "ERROR msvisor"; then
            echo "Error detected in $name"
            results[$name]="failed"
        else
            echo "Command failed for $name"
            results[$name]="failed"
        fi
    done
done

# 输出所有测试结果
echo "Test Results:"
for name in "${!results[@]}"; do
    status=${results[$name]}
    if [ "$status" == "passed" ]; then
        echo -e "\033[32m$name: $status\033[0m"
    else
        echo -e "\033[31m$name: $status\033[0m"
    fi
done

total_tests=${#results[@]}

# 检查通过计数与总测试数量是否相等
if [ "$passed_count" -ne "$total_tests" ]; then
    echo "Error: Not all tests passed!"
    exit 1  # 返回失败状态
fi
