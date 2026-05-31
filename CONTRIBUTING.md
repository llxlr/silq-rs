# 贡献指南

## 开发环境

```bash
# 克隆项目
git clone <repo-url> silq-rs
cd silq-rs

# 构建
cargo build

# 运行测试
cargo test

# 运行特定测试
cargo test lexer
```

## 项目约定

### 代码风格

- 遵循 Rust 标准命名约定 (snake_case, CamelCase)
- 使用 `rustfmt` 格式化 (`cargo fmt`)
- 使用 `clippy` 检查 (`cargo clippy`)

### 模块组织

```
src/
├── main.rs          # 仅 CLI 入口
├── lib.rs           # 库根，公开 API
├── token.rs         # Token 定义和优先级表
├── lexer.rs         # 词法分析 (不应依赖 parser/ast)
├── parser.rs        # 语法分析 (依赖 lexer, ast)
├── ast.rs           # AST 定义 (基础模块)
├── scope.rs         # 符号表
├── semantic.rs      # 语义分析
├── checker.rs       # 线性检查
├── consteval.rs     # 常量求值
├── conversion.rs    # 类型转换
├── reverse.rs       # 逆变换
├── modules.rs       # 模块系统
├── errors.rs        # 错误处理
├── qsim.rs          # 量子模拟器 (核心后端)
├── hqir.rs          # HQIR 后端
└── options.rs       # 编译配置
```

### 测试

- 单元测试放在源码文件底部 (`#[cfg(test)] mod tests`)
- 集成测试放在 `test/*.slq` (Silq 源代码)
- 每个新功能至少包含一个测试

### 提交规范

```
<type>: <简短描述>

<详细说明 (可选)>

类型:
- feat: 新功能
- fix: 错误修复
- refactor: 重构
- docs: 文档
- test: 测试
- chore: 构建/工具
```

## 待实现功能

按优先级排序:

1. **完整的类型推断** — HM(X) 统一算法
2. **运算符降级** — +, *, - 等降级为 __add 函数
3. **完整的逆变换** — 自动伴随函数生成
4. **HQIR 完善** — 量子 IR 格式完整发射
5. **更多量子门** — Toffoli, Fredkin, SWAP
6. **WASM 编译目标** — 浏览器量子模拟
7. **性能优化** — 密集状态向量, SIMD

## 参考

- [Silq 论文 (POPL 2020)](https://silq.ethz.ch)
- [原始 D 实现](https://github.com/tgehr/silq)
- [Rust 官方文档](https://doc.rust-lang.org)
