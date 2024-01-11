# 调研

Rust语言的宏机制是其非常强大和灵活的特性之一。宏允许开发者编写自定义的代码生成器，以便在编译时生成重复使用的代码片段。我们将介绍Rust语言宏的基本概念、语法和用法，深入探讨其实现机制，并分析宏在开源代码仓库中的实际应用。

## 声明宏简介

**宏**（*Macro*）指的是 Rust 中一系列的功能：使用 `macro_rules!` 的**声明**（*Declarative*）宏，和三种**过程**（*Procedural*）宏。宏是一种为写其他代码而写代码的方式，即所谓的 **元编程**（*metaprogramming*），对于减少大量编写和维护的代码是非常有用的。我们首先介绍声明宏。

### 定义

首先看一个例子，一个简化的 `vec` 宏定义如下，从中可以看出大致的语义：

```rust
macro_rules! vec {
    ( $( $x:expr ),* ) => {
        {
            let mut temp_vec = Vec::new();
            $(
                temp_vec.push($x);
            )*
            temp_vec
        }
    };
}
```

声明宏使用 `macro_rules!` 来定义，它允许我们编写一些类似 `match` 表达式的代码，根据匹配的模式对代码做相应的展开。它的语法格式如下：

```rust
macro_rules! $name {
    $rule;
    $rule1;
    // ...
    $ruleN;
}
```

其中至少有一条规则，最后一条规则后的分号可以省略。

使用上下文无关文法的描述如下：
$$
MacroRulesDefinition \to {\rm {'macro\_rules'}} \ {\rm !} \ {\rm IDENTIFIER}\  \{ MacroRules \} \\
MacroRules \to MacroRule \ (; \ MacroRule)^* \ ;^? \\
MacroRule \to MacroMatcher \ {\rm {'\Rarr'}} \ MacroTranscriber
$$

### 规则的定义

每一条规则形如：

``` rust
($matcher) => {$expansion}
```

其中，`matcher` 可以包含字面上的标记（token），如 `fn`，`4`，`“abc"` 等，表示严格匹配这些标记。

`matcher` 还可以包含**捕获**，即基于某种通用语法类别来匹配输入，并将结果捕获到元变量（*Metavariable*）中。

捕获的书写方式是：`$identifier: specifier`，它匹配的输入视分类符 `specifier` 而定，例如 `expr` 匹配一个完整的表达式，`ident` 匹配一个标识符，`ty` 匹配一个类型。

`matcher` 可以有反复捕获 (repetition)，这使得匹配一连串标记成为可能。反复捕获的一般形式为 `$(...) sep rep`，`...` 是被反复匹配的模式，`sep` 是可选的分隔符，`rep` 是重复操作符，`?`、`*`、`+` 分别表示最多一次、零次或多次、一次或多次匹配。

`expansion` 可以是任意的 `token` 序列，表示将捕获到的输入展开为对应序列，其中可以以 `$identifier` 形式调用捕获到的元变量。

### 语义与使用

调用宏的方式与调用函数类似，区别是可以使用全部三种括号，例如 `vec![1, 2]`、`println!("Hello World")`、`lazy_static! { static REF a; }`。

声明宏的语义也与 `match` 表达式类似，在解析时，编译器会选择从上往下第一条匹配的规则，按照 `matcher` 与 `expansion` 的对应关系进行展开。

例如，调用上面定义的 `vec! ` 宏：

```rust
vec![1, 2, 3]
```

会展开为：

```
{
    let mut temp_vec = Vec::new();
    temp_vec.push(1);
    temp_vec.push(2);
    temp_vec.push(3);
    temp_vec
}
```

产生一个包括 `1, 2, 3` 三个元素的 `Vec<i32>`。



## 声明宏的实现机制与细节

