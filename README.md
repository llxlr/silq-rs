# Silq (Rust implementation)

> 用 Rust 重构的 Silq 量子编程语言编译器

Silq 是 ETH Zurich 开发的高阶量子编程语言，具有强静态类型系统和自动去计算(uncomputation)功能。本项目是用 **Rust** 对原始 D 语言实现的完整重写。

## 项目状态

| 模块 | 状态 | 描述 |
|------|------|------|
| 词法分析器 (Lexer) | ✅ 完整 | UTF-8, Unicode 数学符号, 注解, 嵌套注释 |
| 语法分析器 (Parser) | ✅ 完整 | Pratt 表达式解析, 声明, 控制流 |
| AST 定义 | ✅ 完整 | 30+ 表达式节点, 完整类型系统 |
| 语义分析 | ✅ 基本 | 名称解析, 声明注册 |
| 线性检查器 | ✅ 基本 | const/moved 跟踪 |
| 量子模拟器 (QSim) | ✅ 核心 | 状态向量, H/X/Y/Z/CNOT/测量 |
| HQIR 后端 | 🔧 骨架 | 框架就绪 |
| 标准库 | ✅ | prelude.slq 包含 |

**代码规模:** 5,657 行 Rust 代码 | **测试:** 10/10 通过

## 快速开始

### 构建

```bash
cd silq-rs
cargo build --release
```

### 运行

```bash
# 运行量子程序
cargo run -- --run test/bell.slq

# 仅类型检查
cargo run -- --check examples.slq

# 跟踪执行过程
cargo run -- --run --trace test/bell.slq

# dump 量子态
cargo run -- --run --dump test/bell.slq
```

## Silq 语言特性

### 量子/经典类型区分

Silq 用 `!` 前缀标记经典类型，无前缀为量子类型：

```
x: 𝔹          // 量子比特 (qubit)
x: !𝔹         // 经典布尔值 (classical bool)
```

数值类型层级: `𝔹 <: ℕ <: ℤ <: ℚ <: ℝ <: ℂ`

### 基本量子门

```
H(x)          // Hadamard 门
X(x)          // Pauli-X (NOT) 门
Y(x)          // Pauli-Y 门
Z(x)          // Pauli-Z 门
phase(φ)      // 全局相位旋转
CNOT(x, y)    // 受控非门
measure(x)    // 测量量子比特
```

### 示例: Bell 态

```
def main() {
    x := H(0);        // 创建叠加态
    y := 0;
    if x {
        y := X(y);    // CNOT 的 if-实现
    }
    forget(x=y);       // 去计算 x
    assert(!measure(H(y)));  // 验证 Bell 态
}
```

### 示例: Deutsch 算法

```
def solve(const O_f: B x B !-> B x B) {
    x := H(0);
    y := H(1);
    (x, y) := O_f(x, y);
    x := H(x);
    measure(y);
    return measure(x);
}

def main() {
    // 四种 oracle 类型
    def O_id(x: B, y: B) { y xor= x; return (x, y); }
    def O_not(x: B, y: B) { y xor= !x; return (x, y); }
    def O_zero(x: B, y: B) { y xor= 0; return (x, y); }
    def O_one(x: B, y: B) { y xor= 1; return (x, y); }

    assert(solve(O_id));      // id 是平衡的 → 1
    assert(solve(O_not));     // not 是平衡的 → 1
    assert(!solve(O_zero));   // zero 是常数的 → 0
    assert(!solve(O_one));    // one 是常数的 → 0
}
```

## 核心特性

### 1. 自动去计算 (Automatic Uncomputation)

Silq 自动生成量子电路的反向（伴随）变换，实现安全的量子资源释放：

```
def reverse[τ, χ, φ](f: const τ×χ →mfree φ)(c: τ, x: φ): χ {
    f(c, y) := x;
    return y;
}
```

### 2. 线性类型系统

- `const` — 可安全复制（仅限经典值）
- `moved` — 线性使用，恰好一次
- 编译器自动检测违反量子不可克隆定理的操作

### 3. 依赖类型

类型可以依赖值：

```
dat int[n: !ℕ] quantum {}     // 固定宽度整数
def vector[τ](n: !ℕ, x: τ): τ^n  // 固定长度向量
```

## 编译管线

```
源码 (.slq)
    ↓
[Lexer]  → Token 流 (Unicode math, UTF-8)
    ↓
[Parser] → AST (Pratt 表达式解析)
    ↓
[Semantic] → 类型标注 AST (名称解析, 类型推断)
    ↓
[Checker] → 线性资源校验 (const/moved)
    ↓
[Backend]
  ├─ QSim  → 量子模拟执行
  └─ HQIR  → 量子中间表示输出
```

## 命令行选项

```
Usage: silq [OPTION]... [FILE]...

Options:
  --run                运行模拟器
  --compile            编译到 HQIR 格式
  --check              仅静态检查
  --trace              跟踪执行
  --verbose, -v        详细输出
  --dump, --dump-state 执行后输出量子态
  --help, -h           显示帮助
```

## 项目结构

```
silq-rs/
├── Cargo.toml
├── README.md
├── ARCHITECTURE.md
├── library/
│   ├── prelude.slq               # 标准库 (487 行)
│   └── __internal/operators.slq  # 运算符实现
├── src/
│   ├── main.rs                   # CLI 入口 (196 行)
│   ├── lib.rs                    # 库根
│   ├── token.rs                  # Token/关键字/优先级 (348 行)
│   ├── lexer.rs                  # 词法分析器 (759 行)
│   ├── parser.rs                 # Pratt 解析器 (998 行)
│   ├── ast.rs                    # AST 定义 (916 行)
│   ├── scope.rs                  # 符号表 (102 行)
│   ├── semantic.rs               # 语义分析 (269 行)
│   ├── checker.rs                # 线性检查 (153 行)
│   ├── consteval.rs              # 常量求值 (131 行)
│   ├── conversion.rs             # 类型转换 (45 行)
│   ├── reverse.rs                # 量子逆变换 (103 行)
│   ├── modules.rs                # 模块系统 (133 行)
│   ├── errors.rs                 # 错误处理 (272 行)
│   ├── qsim.rs                   # 量子模拟器 (1048 行)
│   ├── hqir.rs                   # HQIR 后端 (84 行)
│   └── options.rs                # 配置 (56 行)
└── test/
    └── bell.slq                  # Bell 态示例
```

## 构建要求

- **Rust** 1.75+
- 依赖项: `num-complex`, `num-bigint`, `itertools`, `colored`

## 参考

- [Silq 官方项目](https://github.com/tgehr/silq)
- [Silq 论文](https://silq.ethz.ch)
- [原始 D 语言实现](https://github.com/tgehr/silq)

## License

Boost Software License 1.0 (与原始项目保持一致)
