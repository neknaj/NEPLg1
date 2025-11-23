# Neknaj Expression Prefix Language - General-purpose 1 の実装方針 / 仕様（starting_detail）

ここに書いてあることは実装前に作成した実装方針です。
実装にあわせて、`doc/` に体系的に分かりやすく纏める必要があります。

このファイルは、「現時点の実装に合わせて更新された仕様の起点」として扱います。

---

## 概要

### 基本的な記法

基本的に全てポーランド記法 / 前置記法で書ける。
これを **P-style** (Polish-style / Prefix-style) と呼ぶ。

```ebnf
<expr> = <prefix> { <expr> }
```

`<expr>` は `()` で囲うことができ、これによって優先度を明示できる。

```ebnf
<parened_expr> = "(" <expr> ")"
<expr>         = <parened_expr>
```

構文解析の段階では「P-style の並び」だけを決め、どこまでが 1 つの関数呼び出しか等は決めない。
**型推論**フェーズで、P-style の列から最終的な呼び出し構造を決定する。

---

## 式指向

様々なものを式 `<expr>` として扱う。
制御構造・ブロック・変数束縛・名前空間・マルチファイル構成など、基本的に全て式。

以下、主な構成要素ごとに仕様を記述する。

---

## 演算子

### 算術演算子・文字列・ベクタ

```ebnf
<math_bin_operator> =
      "add" | "sub" | "mul" | "div" | "mod"
    | "pow"
    | "and" | "or" | "xor" | "not"
    | "lt" | "le" | "eq" | "ne" | "gt" | "ge"
    | "bit_and" | "bit_or" | "bit_xor" | "bit_not"
    | "bit_shl" | "bit_shr"
    | "permutation" | "combination"
    | "gcd" | "lcm"

<string_bin_operator> = "concat" | "get" | "push"
<vec_bin_operator>    = "concat" | "get" | "push"

<bin_operator> = <math_bin_operator> | <string_bin_operator> | <vec_bin_operator>
<expr>         = <bin_operator> <expr> <expr>

<math_un_operator>   = "neg" | "not" | "factorial"
<string_un_operator> = "len" | "pop"
<vec_un_operator>    = "len" | "pop"

<un_operator> = <math_un_operator> | <string_un_operator> | <vec_un_operator>
<expr>        = <un_operator> <expr>
```

これらの演算子は **std lib から関数として提供**する。
コンパイラから見ると単なる関数名であり、特別扱いはしない。

文字列と整数値の橋渡しのために、標準ライブラリには `convert` 名前空間を追加した。
`parse_i32` で 10 進表記の文字列を `i32` に変換し、`to_string` で整数を文字列化する。
`to_bool` は空 / 0 以外を真として 1 / 0 に正規化する。

---

### パイプ演算子 `>`

構文 `LHS > RHS` は、「LHS の結果を RHS の関数の第 1 引数として注入する」糖衣構文である。

```ebnf
<expr>       = <pipe_chain>

<pipe_chain> = <pipe_term> { ">" <pipe_term> }    // 左結合

<pipe_term>  = <expr_without_pipe>
```

`<expr_without_pipe>` は、`>` を含まない通常の式（関数適用・if 式・loop 式・while 式・match 式・ブロック式など）を総称する抽象的な非終端とみなす。

* 結合性: 左結合 (`A > B > C` は `(A > B) > C` と等価)
* 優先順位: P-style の関数適用よりも低い

  * `f x > g` は `(f x) > g` として解釈される

糖衣構文としての展開規則:

* `RHS` が `F A1 A2 ... An` という式のとき、`LHS > RHS` は `F LHS A1 A2 ... An` と等価
* `RHS` が `F` のとき、`LHS > F` は `F LHS` と等価

P-style の連鎖の中に `>` がある場合、`>` の左側のトークン列から、**直近の完結した式だけ** を LHS として切り出す。

---

## 関数

### 関数リテラル式

#### 構文（更新）

```ebnf
<func_literal_expr> =
    "|" <func_literal_args> "|" ( "->" | "*>" ) <type> <expr>

<func_literal_args> =
    { <func_literal_arg> [ "," ] }

<func_literal_arg>  =
    <type> [ "mut" ] <ident>

<expr> = <func_literal_expr>
```

* 一般形

  * `|i32 a, i32 b|->i32 add a b`
  * `|i32 mut x, i32 y|->Unit ...`
  * `|i32 x|*>i32 ...`
* 「**型 → 変数名**」の順で書く。
* `mut` が付いた引数は **mutable 引数** であり、呼び出し元の `let mut` 変数などを **in-out** で受け取る。

#### `->` と `*>`（非純粋関数と純粋関数）

* `->` : **Impure function**（非純粋関数）
* `*>` : **Pure function**（純粋関数）

純粋関数 `*>` に対する制約（ここが元の仕様から拡張された部分）:

1. 引数リスト内に `mut` を含めてはならない。

   * 例: `|i32 mut x|*>i32 ...` は **コンパイルエラー**。
2. 本体内で `set` できるのは、**その関数の中で `let mut` されたローカル変数**（およびそのフィールド）に限る。

   * 外側スコープ（クロージャキャプチャ等）の変数を `set` することはできない。
3. 本体内から呼び出せる関数は、**他の純粋関数（`*>` 型）だけ**。

   * 非純粋関数 `->` 型の値を呼び出すと **コンパイルエラー**。

非純粋関数 `->` では、従来どおり自由度が高い：

* `mut` 引数を使える。
* `set` によって

  * `mut` 引数
  * 外側スコープの `let mut` 変数
    を更新できる（後述の `set` のルールに従う）。
* 純粋関数 `*>` も非純粋関数 `->` も両方呼び出せる。

#### `mut` 引数の意味論（in-out 引数）

`|i32 mut x|->Unit ...` のような **mutable 引数** は、呼び出し元の変数を **in-out** で扱う。

* 呼び出し側では、`mut` 引数には **代入可能な式（assignable）** かつ根本が `let mut` であるものだけが渡せる（後述の `assignable`）。
* 関数本体から見ると、`x` は普通の `let mut x` とほぼ同じように `set x ...` で書き換え可能だが、その結果は呼び出し元の変数に反映される。

※厳密な型規則・別名禁止などの詳細は型システム実装時に詰める。

---

### 関数呼び出し式

```ebnf
<func_call_expr> = <expr> { <expr> }
<expr>           = <func_call_expr>
```

* 一つ目の `<expr>` が **関数値** であり、
* それ以降の `<expr>` が **引数** である。

一つ目の `<expr>` は関数リテラル式であっても `<ident>` であっても `<if_expr>` や `<match_expr>` などを含む任意の `<expr>` であってよい。
その型が関数型（`(T1,...,Tn) -> R` または `(T1,...,Tn) *> R`）であればよい。

---

## if 式 / loop 式 / while 式 / match 式

### if 式

```ebnf
<if_expr> =
    "if" <expr> ["then"] <expr>
    { "elseif" <expr> ["then"] <expr> }
    "else" <expr>
```

* 条件式の型は `Bool` でなければならない。
* 各分岐の式の型から、`Never` 型（後述）を考慮して if 全体の型を決定する。

### loop 式（更新：スコープ式 + 型付きループ）

元仕様では `<loop_expr> = "loop" <expr>` だったが、
`loop` の本体はスコープを持つので `<scoped_expr>` に変更する。

```ebnf
<loop_expr> =
    "loop" <scoped_expr>
```

`loop` は **break で値を返せる型付きループ** として定義する。

#### break / continue の構文（loop/while 共通）

```ebnf
<break_expr> =
    "break" [ <expr> ]

<continue_expr> =
    "continue"

<expr> =
      <break_expr>
    | <continue_expr>
    | ...
```

* `break` / `break expr` / `continue` いずれも **式としての型は `Never`**（後述）。
* ただし `break expr` の `<expr>` 自体には通常の型 `T` が付く。
  `loop` の結果型を決めるために使用される。

#### loop の型規則（型付きループ）

`loop` 本体の中の `break` を見て、`loop` 式全体の型を決める：

1. **値付き `break expr` が 1 つもない場合**
   → `loop` 式の型は **`Unit`**。
2. **値付き `break expr` が 1 つ以上ある場合**
   → それらに現れる `expr` の型はすべて同一 `T` でなければならない。
   → `loop` 式の型は **`T`**。
3. `loop` の型が `T ≠ Unit` の場合、**値なし `break` は使用できない**。
   （値なし `break` は「`Unit` を返すループ」でのみ許される。）

例:

```nepl
; Unit 型のループ
loop:
    if should_stop then
        break
    else
        continue

; i32 を返すループ
let n: i32 =
    loop:
        if cond1 then
            break 10
        elseif cond2 then
            break 20
        else
            continue
```

### while 式（新規追加）

これまで文章中にだけ出ていた `while` を、正式な構文として定義する。

```ebnf
<while_expr> =
    "while" <expr> <scoped_expr>
```

* 条件 `<expr>` の型は `Bool` でなければならない。
* 本体 `<scoped_expr>` 内で何をしても、**`while` 式全体の型は常に `Unit`**。
* 本体内では

  * `break`（値なし）
  * `continue`
  * `return [expr]`
    を使えるが、`break expr`（値付き）は使用できない（`while` は常に `Unit` を返すため）。

### match 式

`match` 式は、共通のスコープ付きリスト `<scoped_list<match_case>>` を用いて定義する。

```ebnf
<match_case> =
    "case" <pattern> "=>" <expr>

<match_expr> =
    "match" <expr> <scoped_list<match_case>>
```

NEPL の `match` は常に `case` と `=>` を用いる。
（`match cmd with | Quit -> ...` のような ML 風記法は使わない。）

`<scoped_list<…>>` のスコープの決定は、後述するスコープの決定方法と同一である。

### if / loop / while / match を `expr` に含める

```ebnf
<expr> =
      <if_expr>
    | <loop_expr>
    | <while_expr>
    | <match_expr>
    | <func_literal_expr>
    | <func_call_expr>
    | <block_expr>
    | <let_expr>
    | <let_function_expr>
    | <include_expr>
    | <import_expr>
    | <namespace_expr>
    | <use_expr>
    | <when_expr>
    | <return_expr>
    | <set_expr>
    | ...        // その他リテラル・識別子等
```

（上記は概略。実際の BNF では適宜合成する。）

---

## パターン

`match` 式で用いる `<pattern>` は、将来的な拡張を見据えて以下のように定義する。
リテラルや変数、ワイルドカードに加えて、`enum` / `struct` 用のパターンも含める。

```ebnf
<pattern> =
      <literal_pattern>
    | <ident_pattern>
    | <wildcard_pattern>
    | <enum_variant_pattern>
    | <struct_pattern>

<literal_pattern>  = <literal>
<ident_pattern>    = <ident>
<wildcard_pattern> = "_"
```

`enum` 用のパターン：

```ebnf
<enum_variant_pattern> =
      <enum_variant_name> "(" <pattern_list> ")"
    | <enum_variant_name>

<pattern_list> = <pattern> ("," <pattern>)*
```

`struct` 用のパターン：

```ebnf
<struct_pattern> =
    <struct_name> "{" <field_pattern_list> "}"

<field_pattern_list> = <field_pattern> ("," <field_pattern>)*
<field_pattern>      = <field_name> ":" <pattern>
```

---

## 型注釈

型注釈によって曖昧な型を決定させたり、書いた式の型が意図したものであるか確認したりできる。

```ebnf
<type_annotation> = <type> <expr>
<expr>            = <type_annotation>
```

---

## スコープ

基本的に、スコープは `{}` (C 風) か `:` (Python 風) によって明示される。

* C 風: `{}` の中身を 1 つのスコープとして扱う
* Python 風: `:` がある行の **次の行から** を 1 つのスコープとして扱い、スコープの中では `:` のある行よりもインデントが大きい（空行のインデントは無視）

この 2 つは併用できる。

```ebnf
<scoped_expr> =
      "{" <expr> "}"
    | ( ":" <expr> with off-side rule )

<expr> = <scoped_expr>
```

### スコープ付きリスト `<scoped_list<…>>`

`match` の `case` 列や、`enum` の variant 列、`struct` の field 列など、
「同一スコープ内に並ぶ要素のリスト」を共通して扱うために、ジェネリックなスコープ付きリスト `<scoped_list<Item>>` を導入する。

```ebnf
<scoped_list<Item>> =
      "{" <item_list<Item>> "}"
    | ( ":" <item_list<Item>> with off-side rule )

<item_list<Item>> =
    { Item ("," | ";") }+
```

* `Item` には `<match_case>` や `<enum_variant>`、`<field>` などを与える
* カンマ `,` とセミコロン `;` の両方を区切り記号として許可する
* `{}` 版と `:` + オフサイドルール版のどちらでも書ける

`match` / `enum` / `struct` は、この `<scoped_list<…>>` を通して同じスコープ決定ロジックを共有する。

---

## 関数のスコープ

関数リテラルでの引数は、そのリテラルの本体 `<expr>` がスコープになる。
引数名はそのスコープ内でのみ有効。

---

## match のスコープ

`match` パターンで使った識別子は、対応する `match_case` がスコープになる。
他の `case` では同名識別子を別に定義してもよい。

---

## ブロック式

複数の式をまとめて式にする。

```ebnf
<block_expr> =
    { <expr> ";" }+ <expr>

<expr> = <block_expr>
```

* `;` の手前の `<expr>` の型が `Unit` でない場合、警告を出してもよい（結果が無視されている）。
* ブロック式の型は **最後の `<expr>` の型**。

---

## 変数束縛式

変数束縛は `let` を基本とし、いくつかの種類がある。
これは型が `Unit` の式である。

```ebnf
<let_suffix> = "mut" | "hoist"
<pub_prefix> = "pub"

<let_expr> =
    [ <pub_prefix> ] "let" [ <let_suffix> ] <ident> [ "=" ] <expr>

<expr> = <let_expr>
```

* 変数束縛は基本的に immutable であり、mutable な変数束縛は `mut` をつけることで作成できる。
* `let mut x = expr`

  * `x` は `set` による再代入が可能な変数。
* `let hoist x = expr`

  * 同一スコープ内であれば、定義の手前でも使用できる（巻き上げ）。
  * `mut` suffix とは共存できず、定数専用。
* `namespace` 直下では `pub` prefix を使用できる（`mut` suffix とは共存できず、公開定数専用）。
* `<ident>` が `_` から始まる場合、未使用エラーを出さない。
* `<ident>` が `_` の場合、実際にはどこにも束縛しない（捨て値）。

### 関数束縛式

関数の定義のために糖衣構文を用意する。

```ebnf
<let_function_expr> =
    [ <pub_prefix> ] "fn" <ident> [ "=" ] <expr>

<expr> = <let_function_expr>
```

これは `let hoist` と同じように扱う。
`<expr>` の型が関数の型でない場合エラーを出す。

### 変数束縛とスコープ

変数束縛が有効なスコープは、変数束縛式がある **もっとも狭いスコープ** になる。

関数リテラル式、if 式、loop 式、while 式、match 式に属する `<expr>` など、
「あって然るべきスコープ」を作成せずに変数束縛式を使用したらエラーを出す。
これは AST でこれらの式から変数束縛式までの間にスコープノードがあるかで判定できる。

---

## 型

式は型を持つ。
式は複数の値を返すことができない。
つまり `() ()` のような `<expr>` は不正であり、`();()` のようにブロック式を用いる必要がある。

型は構造を持つ。例えば `struct` や `enum` や `vec` など。

### 基本型（更新）

組み込みの基本型として、数値型は Rust / WASM と同様に以下を用いる：

* `i32`, `i64` : 32/64 bit 符号付き整数
* `f32`, `f64` : 32/64 bit 浮動小数点数

その他の基本型：

* `Bool` : 真偽値
* `Unit` : 単位型（値を 1 つだけ持つ）
* `Never` : 決して値を返さない型（bottom 型、後述）

数値リテラルのデフォルト：

* 整数リテラル `1` → `i32`
* 小数リテラル `1.0` → `f64`

暗黙の型変換は行わない。
必要なら `cast` 関数などで明示的に変換する。

### 型構文の概略

```ebnf
<type> ::=
      <builtin_type>
    | <type_ident>
    | <qualified_type_ident>
    | <func_type>
    | <type_application>      // ジェネリクス導入時の拡張用

<builtin_type> ::= "i32" | "i64" | "f32" | "f64" | "Bool" | "Unit" | "Never" | ...

<type_ident>           = <ident>
<qualified_type_ident> = <namespace_name> "::" <type_ident>
```

* `enum` や `struct` で定義された型名は `<type_ident>` として参照される。
* `namespace` 内の型は `<qualified_type_ident>` で参照できる。

### Never 型と制御フロー（新規）

**Never 型** は「値を一切持たない bottom 型」である。次のような式に付く：

* `return [expr]`
* `break`
* `break expr`
* `continue`

これらの式は「必ずその位置から通常の実行には戻らない」ため、式としての型を `Never` とする。

性質：

* いかなる型 `T` に対しても `Never <: T`（`Never` はすべての型の部分型）として扱う。

  * `if` / `match` などで、ある分岐が `Never` でも、他の分岐の型から全体の型を決められる。

### 関数型（`->` / `*>`）

```ebnf
<func_type> =
    "(" [ <type> { "," <type> } ] ")" ( "->" | "*>" ) <type>
```

* `(T1, ..., Tn) -> R` : 非純粋関数型
* `(T1, ..., Tn) *> R` : 純粋関数型

純粋関数から呼び出せるのは `*>` 型の関数のみ、という制約を型レベルで表現できる。

### if / match の型付けと Never

`if` / `match` の各分岐の型を `T1, T2, ...` とし、これらの **最小共通 supertype** を `T` とする。

* いずれかの分岐が `Never` 型であっても、`Never` は bottom として扱い、他の分岐の型から `T` を決める。

例：

```nepl
let f = |i32 x|->i32:
    if lt x 0 then
        return 0      ; then: Never
    else
        x             ; else: i32
; if 全体: i32
```

`match` も同様：

```nepl
let g = |Cmd cmd|->Unit:
    match cmd:
        case Quit      => return ()
        case Step(n)   => do_step n
        case Other     => ()
```

* `Quit` アーム: `return () : Never`
* `Step` アーム: `do_step n : Unit`
* `Other` アーム: `() : Unit`

`Never` を bottom として扱うことで、`match` 全体の型を `Unit` と決められる。

### Generics（ジェネリクス）

Generics（ジェネリクス）は、将来導入する型変数付きの多相型。
ここでは詳細な構文は省略し、オーバーロード解決などで単相型が優先される方針のみ維持する。

---

## 複数ファイルサポート

複数ファイルへの自然な分割を行える。
あるファイルの任意の部分をそのまま別ファイルに切り出せる。

別ファイルの読み込みには `include` 式と `import` 式を用いる。

* `import` 式は、その別ファイルを書いた人と使う人が「別」の場合を想定（標準ライブラリやパッケージ）。
* `include` 式は、その別ファイルを書いた人と使う人が「同じ」の場合を想定（同一プロジェクト内の分割）。

どちらも、「その部分をそのファイルの中身で置き換えたかのように扱う」。
ファイルスコープなどはない。
従って、あるファイルの任意の部分をそのまま別ファイルに切り出せる。

ファイル間の循環 include / import は不可。ファイル間の依存関係は DAG。
ただし、エラーメッセージではそれぞれどのファイルのどこかを示すようにするので、
実際にテキストとして単純に置き換えてはいけない。

### include 式

```ebnf
<include_expr> ::= "include" <file_path>
<expr>          = <include_expr>
```

* ファイルパスはそのファイルからの相対パス。
* 型は `Unit`。

### import 式

```ebnf
<import_expr> ::= "import" <import_name>
<expr>        = <import_expr>
```

* ファイルパスはライブラリ側が JSON や CSV で一覧を提供し、それを解決する。
* `import_name` は namespace の構造と似ていることが望ましい。
* 型は `Unit`。

---

## namespace 式

```ebnf
<namespace_expr> ::= [ <pub_prefix> ] "namespace" <namespace_name> <scoped_expr>
<pub_prefix>     = "pub"
```

* `<namespace_name>` は識別子。
* `<namespace_expr>` の `<scoped_expr>` 直下では `pub` prefix を使用できる（さらにネストされた `scoped_expr` では不可）。
* `<namespace_expr>` の型は `Unit`。

namespace 自体も `pub` を付けることで公開・非公開を制御できる。

* `namespace` 内の `namespace` はデフォルトで非公開。
* 外側から参照するには `pub namespace` か `pub use` による再公開が必要。

名前空間の解決は、**現在の namespace からの相対パス** として行う。
ルート（最外）では、ファイルに定義された top-level の namespace や識別子を基点として解決する。

---

## use 式

```ebnf
<use_expr> =
    [ <pub_prefix> ] "use" <use_tree>

<use_tree> =
      <use_path> [ "as" <ident> ]
    | <use_path> "::" "*"

<use_path> =
    <ident> { "::" <ident> }
```

`use` 式は `Unit` 型の式であり、その `use` 式が属するもっとも狭いスコープに、パス `<use_path>` の別名を導入する。

* `use ns1::ns2::func1;` → そのスコープ内で `func1` が使える。
* `use ns1::ns2::func1 as f1;` → そのスコープ内で `f1` が使える。
* `use ns1::ns2::*;` → そのスコープ内で `ns1::ns2` 内の公開された要素が一括で導入される。

`pub use` の場合、その別名は親の namespace からも参照できる（再公開）。

```nepl
namespace ns1 {
    pub namespace ns2 {}
    namespace ns3 {}
    namespace ns4 { fn fn1 hoge }
    pub use ns4;
}

// ルート名前空間から見たとき:

use ns1::ns2;      // OK: ns2 は pub namespace として公開されている
use ns1::ns3;      // error: ns3 は非公開 namespace
use ns2;           // error: ルートから直接 ns2 は見えない
use ns1::ns4::*;   // OK: ns1 内で `pub use ns4;` により再公開されている
```

`use` により導入された名前も、変数束縛と同様に、その式が属するスコープ内のみで有効。

---

## enum, struct

`enum` / `struct` も `namespace` 直下のスコープに現れる宣言であり、
`pub` を付けることで公開することができる。
また、`let hoist` と同様に、型宣言としてスコープ内で前方参照可能（暗黙の hoist）。

```ebnf
<enum_def_expr> =
    [ <pub_prefix> ] "enum" <enum_name> <scoped_list<enum_variant>>

<struct_def_expr> =
    [ <pub_prefix> ] "struct" <struct_name> <scoped_list<field>>

<enum_variant> =
    <enum_variant_name> [ "(" <type_list> ")" ]

<field> = <field_name> ":" <type>
```

* `<enum_name>` は識別子
* `<enum_variant_name>` は識別子
* `<struct_name>` は識別子
* `<field_name>` は識別子
* `<type>` は型

`<enum_def_expr>` の型は `Unit`。
`<struct_def_expr>` の型は `Unit`。

`<scoped_list<enum_variant>>` や `<scoped_list<field>>` は、`match` 式の `<scoped_list<match_case>>` と同じ挙動を持つ：

```nepl
// 例: ブレースを使う書き方
enum Option<T> {
    Some(T);
    None
}

// 例: コロン + インデントを使う書き方
enum Option<T>:
    Some(T)
    None

// struct も同様
struct Point {
    x: i32,
    y: i32,
}

struct Point:
    x: i32
    y: i32
```

### enum / struct と名前解決

* `enum` 名 / `struct` 名は **型名前空間** に登録される。
* `enum` の variant 名は **値名前空間** に登録される（パターンおよびコンストラクタとして使う）。
* `struct` の field 名は、その struct 型に紐づくメタ情報としてのみ保持し、`p.x` 参照時に解決する（トップレベル識別子としては登録しない）。

`match` のパターンで：

```nepl
case Some(x) => ...
case None    => ...
```

や

```nepl
case Point { x: x1, y: _ } => ...
```

といった書き方が自然に可能になる。

---

## 代入 `set`（新規セクション）

### assignable（代入可能式）

`set` の左辺に置ける **代入可能式 (assignable)** を明示的に定義する。

```ebnf
<assignable> =
      <ident>
    | <field_expr>

<field_expr> =
    <expr> "." <ident>
```

将来的にパターン等を拡張する余地を残すが、現在は上記 2 種類。

### set 式

```ebnf
<set_expr> =
    "set" <assignable> <expr>

<expr> = <set_expr>
```

#### 一般ルール

1. 左辺が単純な変数 `x` の場合：

   * `x` は現在のスコープから見える `let mut x` でなければならない。
2. 左辺が `p.x` / `p.x.y` などのフィールドアクセスの場合：

   * 一番外側の基底 `p` は `let mut p` によって束縛された変数でなければならない。
3. `set` 式の型は常に `Unit`。
4. 右辺 `<expr>` の型は、左辺が指す変数 / フィールドの型と一致しなければならない。

例：

```nepl
let mut x = 0
set x 10          ; OK

let y = 0
set y 10          ; エラー: y は immutable

let mut p = Point { x = 1, y = 2 }
set p.x 3         ; OK

let q = Point { x = 1, y = 2 }
set q.x 3         ; エラー: q は immutable
```

### 純粋関数内での `set`

純粋関数 `*>` の本体では、さらに制約がかかる：

* `set` できるのは **その関数のローカルスコープで `let mut` された変数**（およびそのフィールド）のみ。
* 外側スコープの変数（クロージャキャプチャなど）やグローバルを `set` するのは禁止。

---

## マルチプラットフォーム対応

### コンパイル時制約 `when`

```ebnf
<when_expr> ::= "when" "(" <expr> ")" <scoped_expr>
```

* `<expr>` は `Bool` 型であり、**コンパイル時に値が確定する** 必要がある。

`when (cond) block` は、以下のように解釈される：

1. `cond` をコンパイル時に評価する。
2. `cond == true` の場合：`block` の内部を通常どおりパースし、名前解決・型チェック・コード生成を行う。
3. `cond == false` の場合：`block` の内部は無視される（このとき、block 内の未定義シンボル・型エラーなどは報告されない）。

`cond` がコンパイル時に値を持たない式であればコンパイルエラー。

### istarget

コンパイル時関数 `istarget : (String) -> Bool`：

* 引数の `String` が現在のコンパイルターゲット名と一致するなら `true`。
* 一致しないなら `false`。

#### 例

* `istarget "wasm-core"` → Wasm コア用ターゲットなら `true`
* `istarget "wasi"` → WASI 対応ターゲットなら `true`

---

## 処理系の実装

P-style の記法は、関数に限らず、**型推論の段階で木構造が決定される**。

* `<term>` が定数か変数か関数か、引数の数はいくつか等は、構文解析器では扱わない。
* エラーや警告が発生したとしても、適切にエラー復帰を行い、できるだけ全てのエラーを報告できるようにする。

処理系は Rust で実装し、`core` は `no_std` で作成する。
CLI や WebPlayground 用の Wasm など様々なインターフェイスを提供する。
複数ファイルのためのファイル IO 以外は `no_std` で作成できるはずである。
ファイル IO は各プラットフォームに依存する部分として API のような形で抽象化を提供する。

### 処理の流れ

1. 字句解析
2. 構文解析
3. 名前解決・型推論
4. その他チェック

構文解析の段階では P-style の引数や型の解決が行われていない曖昧な構文木を作成し、
型推論の段階では完全に確定した構文木を作成する。

#### 構文解析

構文解析では括弧類、スコープとブロック式、`include` / `import` / `namespace` / `use` 式などの解析・処理を行う。
構文解析の段階で、P-style の引数のような部分は扱わない。

変数束縛は `Unit` の式なので、`let hoge hoge let hoge hoge` のような式は作れないため、
スコープ解析や `;` の存在によって変数束縛の場所の一覧をこの段階で取得できるはずである。

`enum` / `struct` の定義も、この段階で「スコープ内に属する宣言」として収集し、
後段の名前解決で前方参照を許可する（暗黙の hoist 扱い）。

`use` 式についても、この段階で「このスコープで導入されるエイリアス候補」として解析しておき、
名前解決フェーズで実際のパス解決と衝突検出を行う。

#### 名前解決

`hoist` の識別子は、巻き上げが発生するため、定義される場所よりも手前で使用されることがある。
また、相互再帰の関数のように、片方を先に完全に解析することもできないということに留意が必要である。

関数リテラルは引数と返り値の型を先ず提示するので、そこまでを事前に解析することで相互再帰の関数もサポートできるはずである。

`enum` / `struct` については：

* 同一スコープ内のすべての `enum` / `struct` を事前に登録し、型名として前方参照可能にする。
* `enum` の variant 名を値名前空間に登録し、パターン / コンストラクタとして解決する。
* `struct` の field 名はその struct 型に紐づくメタ情報としてのみ保持し、`p.x` 参照時に解決する。

`namespace` と `use` については：

* `namespace` はスコープを作り、`pub` 付きの要素だけが外側から見える。
* `use` 式はそのスコープ内に別名を導入し、`pub use` はその別名を親の namespace に再公開する。
* パス解決は、現在の namespace からの相対パスとして行い、ルートでは top-level から解決する。

#### 型推論

型推論では P-style の記法の引数の決定も扱う。
スタックベースのアルゴリズム（Frame スタック）を用意して処理する。

* 全ての必要な情報は事前に判明するはずなので、手前から処理していけばよい。
* 型注釈も関数も手前に書かれる。

オーバーロードも扱うため、推論中に決定できない型や矛盾する型が現れたときはエラーを出す。
純粋関数 `*>` / 非純粋関数 `->` の違い、`Never` を含む制御フローの整合もここでチェックする。
