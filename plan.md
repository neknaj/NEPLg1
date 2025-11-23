# Neknaj Expression Prefix Language - General-purpose 1 の実装方針

ここに書いてあることは実装前に作成した実装方針です  
実装にあわせて、`doc/`に体系的に分かりやすく纏める必要があります  

## 概要

## 基本的な記法

基本的に全てポーランド記法/前置記法で書ける  
これをP-style(Polish-style/Prefix-style)と呼ぶ  

```ebnf
<expr> = <prefix> { <expr> }
```

`<expr>`は`()`で囲うことができ、これによって優先度を明示できる

```ebnf
<parened_expr> = "(" <expr> ")" 
<expr> = <parened_expr>
```

## 式指向

様々なものを式`<expr>`として扱う

### 演算子

#### 算術演算子

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
<expr> = <bin_operator> <expr> <expr>

<math_un_operator>   = "neg" | "not" | "factorial"
<string_un_operator> = "len" | "pop"
<vec_un_operator>    = "len" | "pop"

<un_operator> = <math_un_operator> | <string_un_operator> | <vec_un_operator>
<expr> = <un_operator> <expr>
```

これらの演算子はstd libから関数として提供する

#### パイプ演算子 `>`

構文 `LHS > RHS` は、「LHS の結果を、RHS の関数の第1引数として注入する」糖衣構文である。

```ebnf
<expr>       = <pipe_chain>

<pipe_chain> = <pipe_term> { ">" <pipe_term> }    // 左結合

<pipe_term>  = <expr_without_pipe>
```

`<expr_without_pipe>` は、`>` を含まない通常の式（関数適用・if式・loop式・match式・ブロック式など）を総称する抽象的な非終端とみなす。

* 結合性: 左結合 (`A > B > C` は `(A > B) > C` と等価)
* 優先順位: P-style の関数適用よりも低い

  * 例: `f x > g` は `(f x) > g` として解釈される

糖衣構文としての展開規則:

* `RHS` が `F A1 A2 ... An` という式のとき、`LHS > RHS` は `F LHS A1 A2 ... An` と等価
* `RHS` が `F` のとき、`LHS > F` は `F LHS` と等価

P-style の連鎖の中に `>` がある場合、`>` の左側のトークン列から、**直近の完結した式** だけを LHS として切り出す。

例:

```neplg1
add 1 add 2 3 > add 4
== add 1 ( (add 2 3) > add 4 )
== add 1 (add 4 (add 2 3))
```

### 関数

#### 関数リテラル式

```ebnf
<func_literal_expr> = "|" <func_literal_args> "|" ( "->" | "*>" ) <type> <expr>
<func_literal_args> = { <func_literal_arg> [ "," ] }
<func_literal_arg>  = <type> <ident>
<expr> = <func_literal_expr>
```

`|引数1,引数2,...|->返り値の型 関数の本体`となる
`->`は普通の関数 `*>`は純粋関数である
関数の本体の型は、返り値の型に一致する

#### 関数呼び出し式

```ebnf
<func_call_expr> = <expr> { <expr> }
<expr> = <func_call_expr>
```

一つ目の`<expr>`が関数であり、それ以降の`<expr>`が引数である
一つ目の`<expr>`は関数リテラル式であっても`<ident>`であっても`<if_expr>`などを挟んだ`<expr>`であっても、型が関数であればいい

### if式/loop式/match式

```ebnf
<if_expr>   = "if" <expr> ["then"] <expr>
              { "elseif" <expr> ["then"] <expr> }
              "else" <expr>

<loop_expr> = "loop" <expr>
```

`match`式は、共通のスコープ付きリスト`<scoped_list<match_case>>`を用いて定義する：

```ebnf
<match_case> = "case" <pattern> "=>" <expr>

<match_expr> = "match" <expr> <scoped_list<match_case>>

<expr> = <if_expr> | <loop_expr> | <match_expr>
```

`<scoped_list<…>>`のスコープの決定は、後述するスコープの決定方法と同一である

### パターン

`match`式で用いる`<pattern>`は、将来的な拡張を見据えて以下のように定義する。
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

`enum`用のパターン：

```ebnf
<enum_variant_pattern> =
      <enum_variant_name> "(" <pattern_list> ")"
    | <enum_variant_name>

<pattern_list> = <pattern> ("," <pattern>)*
```

`struct`用のパターン：

```ebnf
<struct_pattern> =
    <struct_name> "{" <field_pattern_list> "}"

<field_pattern_list> = <field_pattern> ("," <field_pattern>)*
<field_pattern>       = <field_name> ":" <pattern>
```

### 型注釈

型注釈によって曖昧な型を決定させたり、書いた式の型が意図したものであるか確認したりできる

```ebnf
<type_annotation> = <type> <expr>
<expr> = <type_annotation>
```

## スコープ

基本的に、スコープは`{}`(C風)か`:`(Python風)によって明示される

C風では、`{}`の中身を1つのスコープとして扱う
Python風では、`:`がある次の行からを1つのスコープとして扱い、スコープの中では`:`のある行よりもインデントが大きい(ただし空の行のインデントは無視する)
この2つは併用できる

```ebnf
<scoped_expr> = "{" <expr> "}" | ( ":" <expr> with off-side rule )
<expr> = <scoped_expr>
```

### スコープ付きリスト `<scoped_list<…>>`

`match`の`case`列や、`enum`のvariant列、`struct`のfield列など、
「同一スコープ内に並ぶ要素のリスト」を共通して扱うために、
ジェネリックなスコープ付きリスト`<scoped_list<Item>>`を導入する。

```ebnf
<scoped_list<Item>> =
      "{" <item_list<Item>> "}"
    | ( ":" <item_list<Item>> with off-side rule )

<item_list<Item>> =
    { Item ("," | ";") }+
```

* `Item`には`<match_case>`や`<enum_variant>`、`<field>`などを与える
* カンマ`,`とセミコロン`;`の両方を区切り記号として許可する
* `{}`版と`:` + オフサイドルール版のどちらでも書ける

`match` / `enum` / `struct` は、この`<scoped_list<…>>`を通して同じスコープ決定ロジックを共有する。

## 関数のスコープ

関数リテラルでの関数の引数は、そのリテラルの`<expr>`がスコープになる

## matchのスコープ

matchパターンで使った識別子は、対応するmatchアーム(`match_case`)がスコープになる

## ブロック式

複数の式を纏めて式にする

```ebnf
<block_expr> = { <expr> ";" }+ <expr>
<expr> = <block_expr>
```

`;`の手前の`<expr>`の型が`unit`でない場合、警告を出す
ブロック式の型は最後の`<expr>`の型

## 変数束縛式

変数束縛は`let`を基本とし、幾つかの種類がある
これは型が`unit`の式である

```ebnf
<let_suffix> = "mut" | "hoist"
<pub_prefix> = "pub"

<let_expr> =
    [ <pub_prefix> ] "let" [ <let_suffix> ] <ident> [ "=" ] <expr>

<expr> = <let_expr>
```

* 変数束縛は基本的にimmutableであり、mutableな変数束縛は`mut`をつけることで作成できる
* `hoist`を付けることにより、その定義の手前あったとしても、同一スコープ内であれば使用できる(`mut` suffixとは共存できず、定数専用)
* `namespace`直下では`pub` prefixを使用できる(`mut` suffixとは共存できず、定数専用)
* `<ident>`が`_`から始まる場合、未使用エラーを出さない
* `<ident>`が`_`の場合、実際にはどこにも束縛しない

### 関数束縛式

関数の定義のために糖衣構文を用意する

```ebnf
<let_function_expr> =
    [ <pub_prefix> ] "fn" <ident> [ "=" ] <expr>

<expr> = <let_function_expr>
```

これは`let hoist`と同じように扱う
`<expr>`の型が関数の型でない場合エラーを出す

### 変数束縛とスコープ

変数束縛が有効なスコープは、変数束縛式があるもっとも狭いスコープになる

関数リテラル式、if式、loop式、match式、while式に属する`<expr>`など、
あって然るべきスコープを作成せずに変数束縛式を使用したらエラーを出す
これはASTでこれらの式から変数束縛式までの間にスコープノードがあるかで判定できる

## 型

式は型を持つ
式は複数の値を返すことができない
つまり `() ()`のような`<expr>`は不正、`();()`のようにブロック式を用いる必要がある

型は構造を持つ。例えば`struct`や`enum`や`vec`など。

型構文の概略：

```ebnf
<type> ::=
      <builtin_type>
    | <type_ident>
    | <qualified_type_ident>
    | <type_application>      // ジェネリクス導入時の拡張用

<type_ident>           = <ident>
<qualified_type_ident> = <namespace_name> "::" <type_ident>
```

* `enum`や`struct`で定義された型名は`<type_ident>`として参照される
* `namespace`内の型は`<qualified_type_ident>`で参照できる

## 複数ファイルサポート

複数ファイルへの自然な分割を行える
或るファイルの任意の部分をそのまま別ファイルに切り出せる

別ファイルの読み込みにはinclude式とimport式を用いる
import式はその別ファイルを書いた人と使う人が別の場合、include式はその別ファイルを書いた人と使う人が同じ場合を想定する

つまり、import式は標準ライブラリやパッケージマネージャによるライブラリを使用するときに使用する
include式は同じプロジェクトの近いフォルダから読み込むときに使用する

include式、import式ともに、その部分をそのファイルの中身で置き換えたかのように扱う
ファイルスコープなどはない
従って或るファイルの任意の部分をそのまま別ファイルに切り出せる

ファイル間の循環include,importは不可、ファイル間の依存関係はDAGである
ただし、エラーメッセージではそれぞれどのファイルのどこかを示すようにするので、置き換えたかのように扱うだけで、実際にテキストとして単純に置き換えてはいけない

### include式

```ebnf
<include_expr> ::= "include" <file_path>
<expr> = <include_expr>
```

ファイルパスはそのファイルからの相対パス
型は`unit`

### import式

```ebnf
<import_expr> ::= "import" <import_name>
<expr> = <import_expr>
```

ファイルパスはライブラリがjsonやcsvを用いて一覧を提供する必要がある
この一覧を用いてimport_nameからファイルパスを解決する
import_nameはnamespaceの構造と似ていることが望まれる
型は`unit`

### namespace式

```ebnf
<namespace_expr> ::= [ <pub_prefix> ] "namespace" <namespace_name> <scoped_expr>
<pub_prefix>     = "pub"
```

`<namespace_name>`は識別子
`<namespace_expr>`の`<scoped_expr>`直下では`pub` prefixを使用できる
(さらにネストされた`scoped_expr`では不可)
`<namespace_expr>`の型は`unit`

namespace自体も`pub`を付けることで公開・非公開を制御できる。
`namespace`内の`namespace`はデフォルトで非公開であり、外側から参照するには`pub namespace`か`pub use`による再公開が必要である。

名前空間の解決は、**現在のnamespaceからの相対パス**として行う。ルート（最外）では、ファイルに定義されたtop-levelのnamespaceや識別子を基点として解決する。

### use式

```ebnf
<use_expr> =
    [ <pub_prefix> ] "use" <use_tree>

<use_tree> =
      <use_path> [ "as" <ident> ]
    | <use_path> "::" "*"

<use_path> =
    <ident> { "::" <ident> }
```

`use`式は`unit`型の式であり、その`use`式が属するもっとも狭いスコープに、パス`<use_path>`の別名を導入する。

* `use ns1::ns2::func1;` によって、そのスコープ内で `func1` という名前が使える
* `use ns1::ns2::func1 as f1;` によって、そのスコープ内で `f1` という別名が使える
* `use ns1::ns2::*;` によって、そのスコープ内で `ns1::ns2` 内の公開された要素が一括で導入される

`pub use`の場合、その別名は親のnamespaceからも参照できる（再公開）。

```neplg1
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

`use`により導入された名前も、変数束縛と同様に、その式が属するスコープ内のみで有効となる。

### enum, struct

`enum` / `struct` も`namespace`直下のスコープに現れる宣言であり、
`pub`を付けることで公開することができる。
また、`let hoist`と同様に、型宣言としてスコープ内で前方参照可能である(暗黙のhoist)。

```ebnf
<enum_def_expr> =
    [ <pub_prefix> ] "enum" <enum_name> <scoped_list<enum_variant>>

<struct_def_expr> =
    [ <pub_prefix> ] "struct" <struct_name> <scoped_list<field>>

<enum_variant> =
    <enum_variant_name> [ "(" <type_list> ")" ]

<field> = <field_name> ":" <type>
```

* `<enum_name>`は識別子
* `<enum_variant_name>`は識別子
* `<struct_name>`は識別子
* `<field_name>`は識別子
* `<type>`は型

`<enum_def_expr>`の型は`unit`
`<struct_def_expr>`の型は`unit`

`<scoped_list<enum_variant>>`や`<scoped_list<field>>`は、
`match`式の`<scoped_list<match_case>>`と同じ挙動を持つ：

```neplg1
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
    x: Int,
    y: Int,
}

struct Point:
    x: Int
    y: Int
```

#### enum / struct と名前解決

* `enum`名 / `struct`名は **型名前空間** に登録される
* `enum`のvariant名は **値名前空間** に登録される (パターンおよびコンストラクタとして使う)
* `struct`のfield名は、そのstruct型の内部メタデータとしてのみ保持し、
  トップレベルの識別子としては登録しない (`p.x` のようなアクセスを通じてのみ見える)

これにより、`match`のパターンで：

```neplg1
case Some(x) => ...
case None    => ...
```

や

```neplg1
case Point { x: x1, y: _ } => ...
```

といった書き方が自然に可能になる。

## 処理系の実装

P-styleの記法は、関数に限らず、型推論の段階で木構造が決定される
`<term>`が定数であるか変数であるか関数であるか、引数の数は何個か、などは構文解析器では扱わない

エラーや警告が発生したとしても、適切にエラー復帰を行い、できるだけ全てのエラーを報告できるような実装にする

処理系はRustで実装し、`core`はno_stdで作成する
CLIやWebPlayground用のwasmなど様々なインターフェイスを提供する
複数ファイルのためのファイルIO以外はno_stdで作成できるはずである
ファイルIOは各プラットフォームに依存する部分としてAPIのような形で抽象化を提供する

### 処理の流れ

1. 字句解析
2. 構文解析
3. 名前解決, 型推論
4. その他チェック

構文解析の段階ではP-styleの引数や型の解決が行われていない曖昧な構文木を、
型推論の段階では完全に確定した構文木を作成する

#### 構文解析

構文解析では括弧類、スコープとブロック式、include式とimport式、namespace式、use式などの解析・処理を行う。
構文解析の段階で、P-styleの引数のような部分は扱わない。

変数束縛は`unit`の式なので、`let hoge hoge let hoge hoge`のような式は作れないため、
スコープ解析や`;`の存在によって変数束縛の場所の一覧をこの段階で取得できるはずである。

`enum` / `struct` の定義も、この段階で「スコープ内に属する宣言」として収集し、
後段の名前解決で前方参照を許可する(暗黙のhoist扱い)。

`use`式についても、この段階で「このスコープで導入されるエイリアスの候補」として解析しておき、
名前解決フェーズで実際のパス解決と衝突検出を行う。

#### 名前解決

`hoist`の識別子は、巻き上げが発生するため、定義される場所よりも手前で使用されることがある
また、相互再帰の関数のように、片方を先に完全に解析することもできないということに留意が必要である

関数リテラルは引数と返り値の型を先ず提示するので、そこまでを事前に解析することで相互再帰の関数もサポートできるはずである

`enum` / `struct` については：

* 同一スコープ内のすべての`enum`/`struct`を事前に登録し、型名として前方参照可能にする
* `enum`のvariant名を値名前空間に登録し、パターン/コンストラクタとして解決する
* `struct`のfield名はそのstruct型に紐づくメタ情報としてのみ保持し、`p.x`参照時に解決する

`namespace` と `use` については：

* `namespace`はスコープを作り、`pub`付きの要素だけが外側から見える
* `use`式はそのスコープ内に別名を導入し、`pub use`はその別名を親のnamespaceに再公開する
* パス解決は、現在のnamespaceからの相対パスとして行い、ルートではtop-levelから解決する

#### 型推論

型推論ではP-styleの記法の引数の決定も扱う
スタックベースのアルゴリズムを用意して処理する

全ての必要な情報は、事前に判明するはずであるので、手前から処理していけばよい
型注釈も関数も手前に書かれる

オーバーロードも扱うため、推論中に、決定できない型や矛盾する型が現れたときは、エラーを出す