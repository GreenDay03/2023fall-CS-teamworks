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

## 过程宏 Procedural Macros

这是Rust语言的一种特性，允许用户能够拓展Rust编译器

相比于声明式宏的直接Token替换，过程式的宏则是将代码进行一种“再加工”，它的作用对象是代码块的TokenStream.

TokenStream 是一个词法结构，是不包含语义的,结构有些像我们课上所学的AST结果

对于下面的过程宏和调用

```rust
use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn define(attr: TokenStream, item: TokenStream) -> TokenStream {
    eprintln!("attr: {}", attr);
    eprintln!("item: {}", item);
    item
}
```

```rust
use proc_macro_define::define;

#[define(log)]
fn foo() {
    b268tqwrsgfohi;
  println!("Hello, world!");
}

```

执行 ```cargo check ``` 结果如下，可以看出输入进去的流就是对我们使用过程宏的代码块的一种词法分析，在其中加入了```b268tqwrsgfohi ```这样的错误片段，仍然会生成这样的数据

<img src=a.png style="zoom: 67%;" />

``` rust
attr: TokenStream [
    Ident {
        ident: "log",
        span: #0 bytes(41..44),
    },
]
item: TokenStream [
    Ident {
        ident: "fn",
        span: #0 bytes(47..49),
    },
    Ident {
        ident: "foo",
        span: #0 bytes(50..53),
    },
    Group {
        delimiter: Parenthesis,
        stream: TokenStream [],
        span: #0 bytes(53..55),
    },
    Group {
        delimiter: Brace,
        stream: TokenStream [
            Ident {
                ident: "b268tqwrsgfohi",
                span: #0 bytes(62..76),
            },
            Punct {
                ch: ';',
                spacing: Alone,
                span: #0 bytes(76..77),
            },
            Ident {
                ident: "println",
                span: #0 bytes(82..89),
            },
            Punct {
                ch: '!',
                spacing: Alone,
                span: #0 bytes(89..90),
            },
            Group {
                delimiter: Parenthesis,
                stream: TokenStream [
                    Literal {
                        kind: Str,
                        symbol: "Hello, world!",
                        suffix: None,
                        span: #0 bytes(91..106),
                    },
                ],
                span: #0 bytes(90..107),
            },
            Punct {
                ch: ';',
                spacing: Alone,
                span: #0 bytes(107..108),
            },
        ],
        span: #0 bytes(56..110),
    },
]
```

值得注意的是，输出上述结果后，还会输出

```rust
error[E0425]: cannot find value `b268tqwrsgfohi` in this scope
 --> src/main.rs:5:5
  |
5 |     b268tqwrsgfohi;
  |     ^^^^^^^^^^^^^^ not found in this scope
```

的错误信息，因为这段文字是明显的错误，应当是在语法分析的时候检查出来的，而如果在句子结尾处删去```;```,则会在最开始的时候出现这样的错误，

```
error: expected `;`, found `println`
 --> src/main.rs:5:19
  |
5 |     b268tqwrsgfohi
  |                   ^ help: add `;` here
6 |     println!("Hello, world!");
  |     ------- unexpected token
```

因此，我们可以知道，过程宏是在语法分析的时候被处理的，此时，过程宏会获取调用代码的AST,并进行相应的处理

## Rust 宏在实际项目中的使用分析

本文选取多个流行的Rust仓库，分析其中Rust宏使用的模式和频率。

#### [Tokio](https://github.com/tokio-rs/tokio.git)

Tokio是一个可靠、轻量的异步编程库，提供事件驱动的非阻塞IO接口，广泛用于实现网络服务，目前在Github拥有超过2万颗星。

**过程宏**

项目中出现属性式过程宏6次，函数式过程宏2次，均封装在单独的crate tokio-macros中。过程宏虽然数量较少，但是对项目起到了很重要的作用。例如，

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> { ... }
```

实际上是经过了`tokio::main`过程宏的处理，从而对main函数用户实现改写，完成一些初始化工作，实现异步编程的功能。

```rust
#[proc_macro_attribute]
#[cfg(not(test))] // Work around for rust-lang/rust#62127
pub fn main(args: TokenStream, item: TokenStream) -> TokenStream {
    entry::main(args.into(), item.into(), true).into()
}
```

除此之外，`tokio::test`等过程宏完成类似的处理，对测试用例的代码做出一定的转换。

**声明宏**

该仓库中用`macro_rules!`新定义的宏多达170处，我们将其分成若干类别：

- 部分宏接受空字符串作为匹配模板，输出若干函数的实现。例如：

```rust
macro_rules! deref_async_buf_read {
    () => {
        fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
            Pin::new(&mut **self.get_mut()).poll_fill_buf(cx)
        }

        fn consume(mut self: Pin<&mut Self>, amt: usize) {
            Pin::new(&mut **self).consume(amt)
        }
    };
}
```

该宏规定了两个函数的实现，其应用场景仅有两处而且未向外界公开，主要目的是为特定的泛型实现`AsyncBufRead`的特性。此类宏的作用可以简单的理解为节省重复代码的技巧，有利于提升代码质量。

- 部分宏用于批量化的添加项目的属性，例如文档可见性、编译特性(feature)等等，这类宏的目的与第1类相同，例如`cfg_block_on`为项目添加了编译开关属性：

```rust
macro_rules! cfg_block_on {
    ($($item:item)*) => {
        $(
            #[cfg(any(
                    feature = "fs",
                    feature = "net",
                    feature = "io-std",
                    feature = "rt",
                    ))]
            $item
        )*
    }
}
```

- 部分宏承担了一些简单的断言，比较等功能，例如`async_assert_fn_send`，`assert_value`等等，用于测试的框架。这些宏的功能与C语言部分测试框架的功能相同，不再赘述。- 
- 极少数宏可以接受复杂的标记树（token trees，tt），对标记进行复杂的展开，生成新的代码，作为tokio库中重要的功能。例如`join`宏和`select`等。这些宏实现了异步框架的核心功能，体现出Rust语言有强大的元编程能力。以`select`宏为例，该宏允许在多个异步计算中等待，并在单个计算完成后返回。`select`可以接受较复杂的模式`(biased; $p:pat = $($t:tt)* )`和`( $p:pat = $($t:tt)* )`，这些模式被嵌入较复杂的逻辑中，实现select选择的功能。以后者为例，在下列代码中，后一个模式匹配了两个分支，当这两个分支被select的逻辑运行，选中。

```rust
    tokio::select! {
        val = rx1 => {
            println!("rx1 completed first with {:?}", val);
        }
        val = rx2 => {
            println!("rx2 completed first with {:?}", val);
        }
    }
```

#### [RustScan](https://github.com/RustScan/RustScan.git)

RustScan是一个由Rust编写的端口扫描程序，目前在Github上有超过1万颗星。该项目使用Rust宏较少，仅定义6个声明宏，主要是输出一些调试信息或者省略一些字段的填写，总体上来说作用不大。

#### [RustPython](https://github.com/RustPython/RustPython.git)

RustPython是一个用Rust编写的Python3解释器，目前在Github上有超过1万颗星。该项目使用了大量的宏，其中包括：

- 过程宏10处，主要包含`pyclass`等属性宏，`py_compile`等过程宏以及`PyStructSequence`等继承宏。这些过程宏的内容主要是对`derive_impl`模块中同名函数的封装，主要用于实现一些Python的内置类型的方法。

```rust
#[pyattr]
#[pyattr(name = "ArrayType")]
#[pyclass(name = "array")]
#[derive(Debug, PyPayload)]
pub struct PyArray {
    array: PyRwLock<ArrayContentType>,
    exports: AtomicUsize,
}
```

- 声明宏108处，主要可以分为2类：输出函数的实现，例如`impl_from`为符合条件的类批量化添加`from`属性；实现简短功能，例如`ascii`将字面量转化为ascii形式。不同于tokio库，RustPython没有使用声明宏批量化添加项目的属性，也没有使用规模较大、功能较复杂的声明宏。

总的来说，Rust宏发挥着重要的作用。过程宏助力简化复杂逻辑，如用于异步编程框架中的编译时代码生成，减少手动编写重复性工作。声明宏则常被应用于精简代码、统一属性设置和自动化类型定义，提升一致性与维护性。无论是简化特定语法构造还是构建语言扩展，宏都能有效增强代码灵活性和开发效率，广泛应用在各类项目中。