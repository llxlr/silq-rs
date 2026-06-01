# Silq 编译器架构文档

## 概述

本文档描述 Silq Rust 实现的架构设计。原始实现为 D 语言 (~55K 行), 本 Rust 重写保持相同的编译管线架构，但利用 Rust 的类型系统和所有权模型实现更安全和高效的代码。

## 编译管线

```
┌─────────────────────────────────────────────────────────┐
│                    Silq 编译管线                         │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  *.slq  ──→ Lexer ──→ Parser ──→ Semantic ──→ Checker │
│                                     │                   │
│                                     ▼                   │
│                              ┌──────────────┐           │
│                              │   Backend    │           │
│                              ├──────────────┤           │
│                              │ QSim (模拟)  │           │
│                              │ HQIR (IR输出) │           │
│                              │ Check (验证) │           │
│                              └──────────────┘           │
└─────────────────────────────────────────────────────────┘
```

## 模块详解

### 1. Token (token.rs)

**位置:** `src/token.rs` | **行数:** 348

定义所有 token 类型、关键字表和运算符优先级。

**关键设计:**
- 60+ token 类型支持 ASCII 和 Unicode 变体
- `lookup_keyword()` 将标识符映射到关键字 token
- `get_lbp()` 返回运算符的左结合优先级
- 优先级范围: 10 (逗号) ~ 160 (后缀运算符)

**Unicode 映射:**
| 符号 | 含义 | 运算符 |
|------|------|--------|
| → | 函数箭头 | Arrow |
| ⇒ | 胖箭头 (lambda) | FatArrow |
| ← | 赋值 | LeftArrow |
| × | 积类型 | Cross |
| ∧ / ∨ | 逻辑与/或 | And / Or |
| ≤ / ≥ / ≠ | 比较 | Le / Ge / Neq |
| 𝔹 / ℕ / ℤ / ℚ / ℝ / ℂ | 数值类型 | Identifier |

### 2. Lexer (lexer.rs)

**位置:** `src/lexer.rs` | **行数:** 759

将源文本转换为 token 流。

**关键设计:**
- 字节级 UTF-8 处理
- 嵌套块注释 `/+ ... +/` 支持
- 单行注释 `// ...`
- 多种字面量格式: 整数 (十进制/十六进制/二进制), 浮点数, 有理数 (a\b)
- 字符串: 常规 `"..."`, 原始 `r"..."`
- Unicode 数学符号作为单字符 token 或标识符
- 注解: `@[extern="primitive.H"]`

### 3. AST (ast.rs)

**位置:** `src/ast.rs` | **行数:** 916

**核心类型层次:**

```
Expression (枚举, 37 变体)
├── 字面量: Literal
├── 标识符: Identifier
├── 类型: Type { loc, kind: TypeKind }
├── 一元: UnaryPlus, UnaryMinus, LogicalNot, BitwiseNot
├── 二元: Binary { op, left, right }
├── 调用: Call { function, arguments, callee }
├── 控制流: IfThenElse, ForLoop, WhileLoop, Repeat, With
├── 绑定: Let, Assign, Lambda
├── 组合: Compound, Tuple, Vector, Comma, Concat
├── 访问: Index, Slice, Field
├── 注解: TypeAnnotation { kind: Colon|As|Coerce|Pun }
└── 特殊: Forget, Assert, Return, Typeof, Wildcard, Error

Declaration (枚举, 4 变体)
├── VarDecl { name, dtype, vtype, initializer, capture }
├── FunctionDef { name, params, body, annotation, ftype }
├── DatDecl { name, type_params, fields, is_quantum }
└── Import { path }

TypeKind (枚举, 14 变体)
├── Numeric(NumericType)   // 𝔹, ℕ, ℤ, ℚ, ℝ, ℂ
├── FixedInt { bits, signed, classical }
├── ZMod { n, star, classical }
├── Aggregate { name, type_args }
├── Unit, Bottom
├── Tuple(Vec<Expression>), Array(Box<Expression>)
├── Vector { element, length }
├── Product { params, domain, codomain, annotation }  // 函数类型
├── Classical(Box<Expression>)  // !T
├── QNumeric, TypeMeta, Context
└── TypeVar(usize)  // 类型推断变量
```

**标识符系统:**
使用全局 `Interner` (字符串驻留) 实现高效标识符比较:
```rust
pub struct Interner {
    strings: Vec<String>,
    map: HashMap<String, usize>,
}
```

### 4. Parser (parser.rs)

**位置:** `src/parser.rs` | **行数:** 1,045

Pratt 解析器（优先级爬升法），递归下降。

**架构:**
- `nud()` — 前缀/主表达式解析 (标识符, 字面量, 关键字)
- `led()` — 中缀/后缀表达式解析 (二元运算符, 调用, 索引)
- `infix()` — 分发到具体的中缀处理 (含 `:=`/`←`/`+=` 等赋值运算符)
- `parse_expression_precedence(min_bp)` — 优先级爬升主循环

**关键解析方法:**
| 方法 | 用途 |
|------|------|
| `primary()` | 解析原子表达式 |
| `parse_statement()` | 解析声明/语句 |
| `parse_function_def()` | `def f(params): T { body }` |
| `parse_dat_decl()` | `dat Name[params] quantum { fields }` |
| `parse_lambda()` | `λ(params) => body` |
| `parse_if_expr()` | `if cond then e1 else e2` |
| `parse_let()` | `let name := value` |
| `parse_for_expr()` | `for var in range { body }` |
| `parse_with_expr()` | `with ctl do { body }` |
| `parse_compound()` | `{ stmt1; stmt2; ... }` |

### 5. 量子模拟器 (qsim.rs)

**位置:** `src/qsim.rs` | **行数:** 1,097

核心后端，在经典计算机上模拟量子计算。

**关键数据结构:**

```rust
struct QState {
    amplitudes: BTreeMap<BasisState, Amplitude>,  // 稀疏状态向量
    num_qubits: usize,
    variables: HashMap<usize, (usize, usize)>,     // 变量→量子比特映射
    classical_vars: HashMap<usize, Value>,         // 经典变量
}

struct BasisState {
    bits: Vec<u8>,  // 计算基态 (qubit 值)
}

enum Value {            // 运行时值
    Bool, Int, Float, Complex, Rational,
    QVar { index, name },  // 量子变量引用
    Tuple, Array, Closure, Unit, Error,
}
```

**标准量子门:**
| 门 | 矩阵 | 函数 |
|----|------|------|
| H | 1/√2 [[1, 1], [1, -1]] | `hadamard_gate()` |
| X | [[0, 1], [1, 0]] | `pauli_x_gate()` |
| Y | [[0, -i], [i, 0]] | `pauli_y_gate()` |
| Z | [[1, 0], [0, -1]] | `pauli_z_gate()` |
| P(φ) | [[1, 0], [0, e^(iφ)]] | `phase_gate(φ)` |
| Rx(θ) | cos(θ/2)I - i sin(θ/2)X | `rot_x_gate(θ)` |
| Ry(θ) | cos(θ/2)I - i sin(θ/2)Y | `rot_y_gate(θ)` |
| Rz(θ) | [[e^(-iθ/2), 0], [0, e^(iθ/2)]] | `rot_z_gate(θ)` |

**Interpreter:**
`struct Interpreter` 遍历语义分析的 AST 并对 `QState` 执行操作：
- `eval(Expression) → Result<Value, String>` — 递归 AST 求值
- `call_function(Declaration, args) → Value` — 函数调用
- `call_builtin(name, args) → Value` — 内置量子门
- `alloc_qubit()` — 分配新 qubit 并初始化态向量 `|0⟩`
- 测量使用 `fastrand` 库实现真随机数生成

### 6. 其他关键模块

| 模块 | 文件 | 功能 |
|------|------|------|
| Scope | scope.rs | 嵌套符号表，唯一名称生成 |
| Semantic | semantic.rs | 名称解析，类型推断准备 |
| Checker | checker.rs | 线性资源使用验证 |
| ConstEval | consteval.rs | 编译时表达式求值/常量折叠 |
| Conversion | conversion.rs | 数值类型隐式转换 |
| Reverse | reverse.rs | 量子电路的自动逆变换 |
| Modules | modules.rs | 模块导入/缓存，prelude 加载 |
| Errors | errors.rs | 错误处理终端/JSON 双模式 |
| HQIR | hqir.rs | 量子 IR 文本输出后端 |
| Options | options.rs | 编译器配置 |

## 数据流

```
Source code (String)
    ↓ Lexer::new(source).tokenize_all()
Vec<Token>
    ↓ Parser::new(lexer, interner).parse_program()
Vec<Expression>  (AST)
    ↓ SemanticAnalyzer::new(interner, scope).semantic_program()
Vec<Expression>  (类型标注 AST)
    ↓ Checker::new().check_function()
bool  (线性检查通过/失败)
    ↓ QSim::new(interner).run(ast)
Value           (计算结果)
```

## 设计决策

### 1. 表达式即类型

在 Silq 中，类型就是表达式（依赖类型），因此 Rust AST 中
`Expression` 和类型系统共用 `Expression::Type(TypeKind)` 变体。

### 2. 稀疏状态向量

使用 `BTreeMap<BasisState, Amplitude>` 而非密集数组，因为：
- 量子程序的离散部分可能有大量零振幅项
- 条件分支产生不同的计算基态组合

### 3. 标识符驻留

`Interner` 将字符串映射到 `Id(usize)`，使标识符比较为 O(1)
的整数比较，这对哈希表和模式匹配至关重要。

### 4. 模块系统

`include_str!("../library/prelude.slq")` 在编译时将标准库嵌入
二进制文件，无需运行时文件查找。

## 与原始 D 实现的对比

| 方面 | D 实现 | Rust 实现 |
|------|--------|-----------|
| 代码量 | ~55,000 行 | ~5,754 行 |
| AST 节点 | 类层次 (虚函数) | 枚举 + match |
| 内存管理 | GC | 所有权系统 |
| 错误处理 | 异常 | Result<T, E> |
| 并发 | 无 | 可安全添加 |
| 测试 | 800+ 集成测试 | 10 单元测试 + 待扩展 |
| 类型推断 | 完整 (统一算法) | 基本 (名称解析) |
| 逆变换 | 完整 | 框架实现 |
| 运算符降级 | 完整 | 未实现 |

## 性能特征

- **状态向量大小:** O(2^n) 其中 n = qubit 数
- **门操作:** O(2^n) 每次操作
- **内存:** 每非零振幅 ~24 字节
- **实际限制:** ~25 qubits 在 16GB RAM 上

## 扩展方向

1. **完整类型推断** — 实现 HM(X) 或类似统一算法
2. **运算符降级** — 将 `+`, `*`, `-` 等降级为 `__add`, `__mul` 函数调用
3. **逆变换** — 完整的自动伴随函数生成
4. **HQIR 完善** — 完整的量子 IR 发射
5. **性能优化** — 密集状态向量, SIMD 加速
6. **分布式模拟** — 多节点状态向量分片
7. **WASM 目标** — 浏览器内量子模拟
