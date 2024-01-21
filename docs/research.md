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
MacroRulesDefinition \to {\rm {'macro \textunderscore rules'}} \ {\rm !} \ {\rm IDENTIFIER}\  \{ MacroRules \}
$$

$$
MacroRules \to MacroRule \ (; \ MacroRule)^* \ ;^?
$$

$$
MacroRule \to MacroMatcher \ {\rm {'\Rightarrow'}} \ MacroTranscriber
$$

### 规则的定义

每一条规则形如：

``` rust
($matcher) => {$expansion}
```

其中，`matcher` 可以包含字面上的标记（token），如 `fn`，`4`，`“abc"` 等，表示严格匹配这些标记。

`matcher` 还可以包含**捕获**，即基于某种通用语法类别来匹配输入，并将结果捕获到元变量（*Metavariable*）中。

捕获的书写方式是：`$identifier: specifier`，它匹配的输入视分类符 `specifier` 而定，例如 `expr` 匹配一个完整的表达式，`ident` 匹配一个标识符，`ty` 匹配一个类型，`literal` 匹配一个字面值。

`matcher` 可以有反复捕获 (repetition)，这使得匹配一连串标记成为可能。反复捕获的一般形式为 `$(...) sep rep`，`...` 是被反复匹配的模式，`sep` 是可选的分隔符，`rep` 是重复操作符，`?`、`*`、`+` 分别表示最多一次、零次或多次、一次或多次匹配。

`expansion` 可以是任意的 `token` 序列，表示将捕获到的输入展开为对应序列，其中可以以 `$identifier` 形式调用捕获到的元变量。

### 语义与使用

调用宏的方式与调用函数类似，区别是可以使用全部三种括号，例如 `vec![1, 2]`、`println!("Hello World")`、`lazy_static! { static REF a; }`。

声明宏的语义也与 `match` 表达式类似，在解析时，编译器会选择从上往下第一条匹配的规则，按照 `matcher` 与 `expansion` 的对应关系进行展开。

例如，调用上面定义的 `vec!` 宏：

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



## 宏的实现机制与细节

上面的介绍省略了一些细节。这是因为想要详细了解它们，必须先了解当前 Rust 编译器处理宏语法扩展的一般机制。

### Rust 语言的编译流程

和我们在课上熟知的一样，Rust 语言的编译也有词法分析、语法分析、类型检查、中间代码生成等多个阶段。Rust 的宏机制是语法级别的，因此我们关注两个阶段。

根据 [`Rust Compiler Development Guide`](https://rustc-dev-guide.rust-lang.org/the-parser.html) 的介绍，[`rustc_lexer`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_lexer/index.html) 是词法分析器，将源程序转变为 token 流。Rust 语言的 token 包括标识符（identifier），字面值（literal），关键字（keyword），符号（symbol）等。可以看到，其中有一部分和上面的原变量有对应关系。

C/C++ 语言的宏机制就作用在词法分析阶段，预处理器直接进行简单的替换操作。这意味着，如果不细心使用，可能会出现一些意料之外的问题，例如最经典的运算符优先级问题：

```cpp
#define add(x, y) x + y
int main() {
    return 3 * add(1, 1 << 1);
}
```

这里宏展开时被直接替换，实际的返回值是 `3 * 1 + 1 << 1`，等于 `((3 * 1) + 1) << 1`，而非我们期望的 `3 * (1 + (1 << 1))`（如果使用函数实现 `add`，得到的也是我们期望的结果！）。对于这个问题使用加括号的方式，将宏定义改为 `((x) + (y))` 即可解决。与之类似的，包含多条语句的宏会出现下面问题：

```cpp
#define swap(x, y)  \
        int t = x;  \
        x = y;      \
        y = t;
int main() {
    int a[] = {1, 2, 3}, t = 1;
    swap(a[1], a[2]);
    if (a[0] == a[1]) swap(a[0], a[1]);
}
```

第一次调用宏，会造成变量 `t` 的重复定义。而第二次调用，宏内部只有第一条语句在 `if` 分支内，剩下两句在主函数中，与期望行为不同。类似地，在最外层加大括号可以解决这个问题。但加括号后，就无法在宏的内部定义之后需要用到的变量。当我们需要让一部分变量暴露出来，一部分变量保留在宏内部时，就需要很复杂的处理。

而 Rust 语言的宏机制作用在语法分析之后，因此我们不需要额外注意优先级的问题。（对于第二个问题，我们之后将看到宏机制的部分卫生性是如何解决它的。）[`rustc_parse`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_parse/index.html) 是 Rust 语言的语法分析器，将 token 流进一步转化为抽象语法树（[`rustc_ast`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_ast/index.html)）。AST 包含了更多的语义信息，例如 BinOp，LitInt 等。

### 宏的展开

虽然宏的处理发生在 AST 生成之后，实际上它操作的仍然是一颗标记树（token tree），而非语法树。标记树介于标记流和 AST 之间，具体地，它仅仅是考虑括号结构的标记流，例如 `a + b + (c + d[0]) + e` 对应的标记树是：

```text
«a» «+» «b» «+» «(   )» «+» «e»
          ╭────────┴──────────╮
           «c» «+» «d» «[   ]»
                        ╭─┴─╮
                         «0»

```

在第一遍语法分析时，编译器并不对宏的输入、输出进行任何假设。在 AST 中，它们不会被进一步解析——即使其中看起来包含了各种各样的 Rust 代码——而是以 token tree 的形式停留在叶节点中。这其实带来了更多的灵活性，因为这意味着宏的输入和输出都并不需要是一个合法的 Rust 语句块，而可以使任何（括号匹配的）token 序列。

在生成 AST 之后、编译器对程序进行语义理解之前，编译器将会对所有语法拓展进行展开。编译器遍历 AST，每遇到一个宏调用，就会按照它对 token tree 的操作规则展开，并对展开后的内容进行解析。解析得到的结果会根据上下文被作为 AST 中的一个节点（例如在一个表达式节点内部调用宏，展开结果也会被作为表达式节点），完全替换宏调用处的叶节点。这个过程可以嵌套发生。正因为在 AST 上完全替换的操作，Rust 宏的解析与函数调用很相似，不需要担心优先级等问题。

### 宏的卫生性

课上提及了卫生宏的概念。简而言之，宏的调用不应该干扰上下文，由语法扩展创建的标识符不能被调用该语法扩展的环境访问，语法扩展也不能引用到在语法扩展之外定义的内容。

对局部变量而言，Rust 的声明宏是卫生的，内外环境间相互隔离。看起来有些意外的是，如果一个标识符是由调用传入的，那么在宏中由该标识符创建的变量在上下文中可以访问，这与 Rust 对卫生性的实现有关；它允许我们在宏中有选择的暴露出部分变量。

在课件给出的论文 *A Theory of Hygienic Macros* 中，宏机制使用一种更换名称的策略实现。而包括在 Rust 在内一些语言的实现则有所不同。在 Rust 语言中，每个标识符都被赋予一个语法上下文，只有在名称和上下文都一样时，才被视为同一个标识符；而宏的每次展开都会产生一个新的上下文，因此，相同名称的标识符之间不会互相混淆。

而元变量在捕获时会将上下文一并捕获。因此，在宏内部使用元变量定义的变量和在外部的被视为等同，在外部仍可以使用该变量。考虑下面的例子：

```rust
macro_rules! hygiene {
    ($a: ident) => {
        let $a = 42;
        let b = 21;
    }
}

fn main() {
    hygiene!(a);
    println!("{a} {b}");
}
```

使用 [Rust Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021) 对宏进行展开：

```rust
fn main() {
    let a = 42;
    let b = 21;
    println!("{a} {b}");
}
```

看起来这段程序完全正确，但两个 `b` 的上下文是不同的，因此 `println!` 不能找到需要的 `b`，报错 *cannot find value `b` in this scope*。而 `a` 被元变量捕获，因此是同一个上下文，可以正常使用。基于这样的机制，我们就可以通过传入变量名的方式，让宏只暴露之后需要使用的变量。

我们举一个实际应用中的例子。我们需要以流式处理输入，现已存在 `Scanner` 类，使用它的 `next` 方法（泛型）即可从标准输入流中读取一个值。若我们的需求是连续读取多个变量，就需要对每个变量写一遍 `let a = scanner.next::<T>();`。我们需要使用宏来去除重复代码，并且由于 `scanner` 仅有这一个用途，我们不希望它被暴露出来。代码的一部分如下：

```rust
macro_rules! io_prelude {
    () => {
        let mut scanner = Scanner::new();
        macro_rules! input {
            ($$($ident:ident : $type:tt),+ ) => {
                $$(let $ident = scanner.next::<$type>();)+
            };
        }
    }
}

fn main() {
    io_prelude!();
    input!{ a: usize, b: i32 }
    // let c = scanner.next::<i64>(); ERROR!
    println!("{a} {b}");
}
```

（其中 `$$` 是一些细节问题造成的转义，与 `$` 等同。）可以看到，在调用 `io_prelude!` 宏之后，`scanner` 变量始终存在并且可以通过 `input!` 宏继续使用，但由于它在不同的上下文，无法直接通过变量名访问它，这起到了封装的作用。