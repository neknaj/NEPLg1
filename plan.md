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
<expr> = <parened_expr>>
```

## 式指向
様々なものを式`<expr>`として扱う  
### 演算子
#### 算術演算子
```ebnf
<math_bin_operator> = "add" | "sub" | "mul" | "div" | "mod" | "pow" | "and" | "or" | "xor" | "not" | "lt" | "le" | "eq" | "ne" | "gt" | "ge" | "bit_and" | "bit_or" | "bit_xor" | "bit_not" | "bit_shl" | "bit_shr" | "permutation" | "combination" | "gcd" | "lcm"
<string_bin_operator> = "concat" | "get" | "push"
<vec_bin_operator> = "concat" | "get" | "push"
<bin_operator> = <math_bin_operator> | <string_bin_operator> | <vec_bin_operator>
<expr> = <bin_operator> <expr> <expr>

<math_un_operator> = "neg" | "not" | "factorial"
<string_un_operator> = "len" | "pop"
<vec_un_operator> = "len" | "pop"
<un_operator> = <math_un_operator> | <string_un_operator> | <vec_un_operator>
<expr> = <un_operator> <expr>
```
これらの演算子はstd libから関数として提供する  
### 関数
#### 関数リテラル式
```ebnf
<func_literal_expr> = "|" <func_literal_args> "|" ( "->" | "*>" ) <type> <expr>
<func_literal_args> = { <func_literal_arg> [ "," ] }
<func_literal_arg> = <type> <ident>
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
<if_expr> = "if" <expr> ["then"] <expr> { "elseif" <expr> ["then"] <expr> } "else" <expr>
<loop_expr> = "loop" <expr>
<match_expr> = "match" <expr> { "case" <pattern> "=>" <expr> [ "," ] }
<expr> = <if_expr> | <loop_expr> | <match_expr>
```
ここで、`{ "case" <pattern> "=>" <expr> }`の部分の範囲の決定に、スコープの範囲の決定方法を準用する(構文解析の都合による)  

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

## 関数のスコープ
関数リテラルでの関数の引数は、そのリテラルの`<expr>`がスコープになる  
## matchのスコープ
matchパターンで使った識別子は、対応するmatchアームがスコープになる  

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
```
<let_suffix> = "mut" | "hoist"
<let_expr> = <pub_prefix> "let" [ <let_suffix> ] <ident> [ "=" ] <expr>
<expr> = <let_expr>
```
変数束縛は基本的にimmutableであり、mutableな変数束縛は`mut`をつけることで作成できる  
`hoist`を付けることにより、その定義の手前あったとしても、スコープ内であれば使用できる(`mut` suffixとは共存できず、定数専用)  
namespace直下では`pub` prefixを使用できる(`mut` suffixとは共存できず、定数専用)  
`<ident>`が`_`から始まる場合、未使用エラーを出さない  
`<ident>`が`_`の場合、実際にはどこにも束縛しない  
### 関数束縛式
関数の定義のために糖衣構文を用意する  
```
<let_fnction_expr> = <pub_prefix> "fn" <ident> [ "=" ] <expr>
<expr> = <let_fnction_expr>
```
これは`let hoist`と同じように扱う  
<expr>の型が関数の型でない場合エラーを出す  

### 変数束縛とスコープ
変数束縛が有効なスコープは、変数束縛式があるもっとも狭いスコープになる  

関数リテラル式、if式、loop式、match式、while式に属する`<expr>`など、あって然るべきスコープを作成せずに変数束縛式を使用したらエラーを出す  
これはASTでこれらの式から変数束縛式までの間にスコープノードがあるかで判定できる  

## 型
式は型を持つ  
式は複数の値を返すことができない  
つまり `() ()`のような`<expr>`は不正、`();()`のようにブロック式を用いる必要がある  
型は構造を持つ 例えばstructやenumやvecなど

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
この一覧を用いてimpert_nameからファイルパスを解決する  
import_nameはnamespaceの構造と似ていることが望まれる  
型は`unit`

### namespace式
```ebnf
<namespace_expr> ::= "namespace" <namespace_name> <scoped_expr>
<pub_prefix> = "pub"
```
`<namespace_name>`は識別子
`<namespace_expr>`の`<scoped_expr>`直下では`pub` prefixを使用できる(さらにネストされたscoped_exprでは不可)  
`<namespace_expr>`の型は`unit`

## 処理系の実装
P-styleの記法は、関数に限らず、型推論の段階で木構造が決定される  
<term>が定数であるか変数であるか関数であるか、引数の数は何個か、などは構文解析器では扱わない  

エラーや警告が発生したとしても、適切にエラー復帰を行い、できるだけ全てのエラーを報告できるような実装にする  

処理系はRustで実装し、`core`はno_stdで作成する  
CLIやWebPlayground用のwasmなど様々なインターフェイスを提供する  
複数ファイルのためのファイルIO以外はno_stdで作成できるはずである ファイルIOは各プラットフォームに依存する部分としてAPIのような形で抽象化を提供する  

### 処理の流れ
1. 字句解析
2. 構文解析
3. 名前解決,型推論
4. その他チェック

構文解析の段階ではP-styleの引数や型の解決が行われていない曖昧な構文木を、  
型推論の段階では完全に確定した構文木を作成します  
#### 構文解析
構文解析では括弧類、スコープとブロック式、include式とimport式の解析、処理を行う  
構文解析の段階で、P-styleの引数のような部分は扱わない  
変数束縛は`unit`の式なので、`let hoge hoge let hoge hoge`のようは式は作れないため、スコープ解析や`;`の存在によって変数束縛の場所の一覧をこの段階で取得できるはずである  
#### 名前解決
`hoist`の識別子は、巻き上げが発生するため、定義される場所よりも手前で使用されることがある  
また、相互再帰の関数のように、片方を先に完全に解析することもできないということに留意が必要である  
関数リテラルは引数と返り値の型を先ず提示するので、そこまでを事前に解析することで相互再帰の関数もサポートできるはずである  
#### 型推論
型推論ではP-styleの記法の引数の決定も扱う  
スタックベースのアルゴリズムを用意して処理する  
全ての必要な情報は、事前に判明するはずであるので、手前から処理していけばよい  
型注釈も関数も手前に書かれる  
オーバーロードも扱うため、推論中に、決定できない型や矛盾する型が現れたときは、エラーを出す  