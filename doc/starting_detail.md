# Neknaj Expression Prefix Language (NEPL) — Starting Detail

この文書は、Neknaj Expression Prefix Language（以下 NEPL）の **現時点で合意済みの仕様** を 1 ファイルにまとめた「スタート地点の仕様書」です。

将来 `doc/` 以下に細かく分割する前の、**単一ソース・オブ・トゥルース**として扱います。

* ファイル拡張子: `.nepl`
* 本文中では、一部に英語の専門用語を用い、その最初の登場時に形式化された表記で示します。

---

# 1. 言語の基本方針

## 1.1 P-style（前置記法）

**Prefix notation** (`/ˈpriː.fɪks noʊˈteɪ.ʃən/`, 前置記法 [プレフィックスきほう]) をベースとした式指向言語。

```ebnf
<expr> = <prefix> { <expr> }
```

* すべての式は「**先頭に演算子（関数）→ 後ろに引数が連続**」という形で書く。
* `()` で囲って優先順位・グルーピングを明示できる：

```ebnf
<parened_expr> = "(" <expr> ")"
<expr>         = <parened_expr>
```

構文解析（パーサ）は **P-style の「並び」だけ決めて、木構造は型推論フェーズで決定** する。

## 1.2 式指向

* 制御構造（if, loop, match）、ブロック、変数束縛、include/import/namespace など、ほぼすべてが式 `<expr>`。
* 式は必ず **型** を持ち、**複数の値を同時に返さない**（タプルで代用）。

## 1.3 処理系パイプライン

**Compiler pipeline** (`/kəmˈpaɪ.lɚ ˈpaɪp.laɪn/`, 処理系パイプライン) は次の段階を想定：

1. **Lexical analysis** (`/ˈlek.sɪ.kəl əˈnæl.ə.sɪs/`, 字句解析)
2. **Parsing** (`/ˈpɑːr.sɪŋ/`, 構文解析)
3. **Name resolution & Type inference** (`/neɪm ˌrez.əˈluː.ʃən/`, 名前解決; `/taɪp ˈɪn.fər.əns/`, 型推論)
4. その他チェック（未使用警告など）
5. Codegen（初期ターゲットは WebAssembly）

重要なポイント:

* 構文解析では **P-style の「どこまでが 1 つの関数呼び出しか」「どの識別子が関数か」などは決めない**。
* 型推論フェーズで、**P-style の木構造決定 + オーバーロード解決** を一気に行う。

## 1.4 実装言語とターゲット

* 実装言語: Rust
* `core` 部は `no_std` で実装する。
* CLI / WebPlayground（Wasm）など複数のフロントエンドを想定。
* ファイル IO などプラットフォーム依存部は抽象化した API 経由で扱う。

---

# 2. 構文と主要構成要素

## 2.1 演算子（算術・論理・文字列・ベクタ）

演算子はすべて **通常の関数として標準ライブラリから提供** される。

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

コンパイラ的には `add`, `sub` などは単なる名前であり、実装は標準ライブラリ側（`std.math`）が担う。

## 2.2 パイプ演算子 `>`

**Pipe operator**（`/paɪp ˈɑː.pəˌreɪ.t̬ɚ/`, パイプ演算子）は糖衣構文：

```ebnf
<expr>       = <pipe_chain>

<pipe_chain> = <pipe_term> { ">" <pipe_term> }    // 左結合

<pipe_term>  = <expr_without_pipe>
```

* 左結合: `A > B > C` は `(A > B) > C` と等価。
* 優先順位: P-style の関数適用よりも低い。

  * `f x > g` は `(f x) > g` と解釈。

**展開規則**:

* `RHS` が `F A1 A2 ... An` なら `LHS > RHS` は `F LHS A1 A2 ... An`。
* `RHS` が `F` だけなら `LHS > F` は `F LHS`。

P-style の長い列の中に `>` がある場合、**直近の完結した式を LHS として切り出す**。

## 2.3 関数リテラル & 関数呼び出し

### 2.3.1 関数リテラル

```ebnf
<func_literal_expr> = "|" <func_literal_args> "|" ( "->" | "*>" ) <type> <expr>
<func_literal_args> = { <func_literal_arg> [ "," ] }
<func_literal_arg>  = <type> <ident>
<expr>              = <func_literal_expr>
```

* `|a: Int, b: Int|->Int body` のように書く。
* `->` は通常の関数、`*>` は「純粋関数」として扱う予定（実装詳細は後続 doc）。
* 本体 `<expr>` の型は宣言された戻り値の型と一致しなければならない。

### 2.3.2 関数呼び出し

```ebnf
<func_call_expr> = <expr> { <expr> }
<expr>           = <func_call_expr>
```

* 先頭の `<expr>` が関数値、それ以降の `<expr>` が引数。
* 先頭は関数リテラルでも `ident` でも `if` 式などの結果でもよい。型が関数であれば OK。

## 2.4 制御構造: if / loop / match

```ebnf
<if_expr>   = "if" <expr> ["then"] <expr>
              { "elseif" <expr> ["then"] <expr> }
              "else" <expr>

<loop_expr> = "loop" <expr>
```

match 式は共通のスコープ付きリスト `<scoped_list<match_case>>` で表現：

```ebnf
<match_case> = "case" <pattern> "=>" <expr>

<match_expr> = "match" <expr> <scoped_list<match_case>>

<expr> = <if_expr> | <loop_expr> | <match_expr>
```

## 2.5 パターン

**Pattern**（`/ˈpæt.ɚn/`, パターン）は将来の拡張も見据えた構造：

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

Enum 用:

```ebnf
<enum_variant_pattern> =
      <enum_variant_name> "(" <pattern_list> ")"
    | <enum_variant_name>

<pattern_list> = <pattern> ("," <pattern>)*
```

Struct 用:

```ebnf
<struct_pattern> =
    <struct_name> "{" <field_pattern_list> "}"

<field_pattern_list> = <field_pattern> ("," <field_pattern>)*
<field_pattern>      = <field_name> ":" <pattern>
```

## 2.6 型注釈

```ebnf
<type_annotation> = <type> <expr>
<expr>            = <type_annotation>
```

* 任意の式に対して `Int expr` のように型注釈を付けられる。
* 型推論を補助し、意図した型と一致しない場合にエラーとして検出する。

---

# 3. スコープとブロック

## 3.1 スコープ構文

スコープは **C 風 `{}`** と **Python 風 `:` + インデント** の 2 種類で表現できる。

```ebnf
<scoped_expr> = "{" <expr> "}" | ( ":" <expr> with off-side rule )
<expr>        = <scoped_expr>
```

* `{}` 版: `{ ... }` 内部が 1 スコープ。
* `:` 版: `:` の次の行以降で、**インデントが増えた行** がスコープの中身になる（空行のインデントは無視）。
* 両者は併用可能。

## 3.2 スコープ付きリスト `<scoped_list<…>>`

**Scoped list**（スコープ付きリスト）は、`match` / `enum` / `struct` などで共通する構文コンビネータ：

```ebnf
<scoped_list<Item>> =
      "{" <item_list<Item>> "}"
    | ( ":" <item_list<Item>> with off-side rule )

<item_list<Item>> =
    { Item ("," | ";") }+
```

* `Item` に `<match_case>` / `<enum_variant>` / `<field>` などを与える。
* カンマ`,`とセミコロン`;` の両方を区切りとして許可。

## 3.3 ブロック式

```ebnf
<block_expr> = { <expr> ";" }+ <expr>
<expr>       = <block_expr>
```

* 複数の式を `;` でつないで 1 つの式にする。
* 先頭〜末尾ひとつ前までの式の型が `Unit` 以外なら警告を出す。
* ブロック式全体の型は **最後の `<expr>` の型**。

## 3.4 変数束縛式 `let`

```ebnf
<let_suffix> = "mut" | "hoist"
<pub_prefix> = "pub"

<let_expr> =
    [ <pub_prefix> ] "let" [ <let_suffix> ] <ident> [ "=" ] <expr>

<expr> = <let_expr>
```

ルール:

* デフォルト: immutable
* `mut` 付き: mutable 変数束縛
* `hoist` 付き: 巻き上げ（同一スコープ内なら定義より前でも使用可）

  * `mut` とは共存不可（定数専用）
* `namespace` 直下では `pub` prefix を使用可（`mut` とは共存不可、定数専用）
* `<ident>` が `_` で始まる場合、未使用でも警告を出さない。
* `<ident>` が `_` そのものの場合、実際にはどこにも束縛しない「捨てパターン」。

### 3.4.1 関数束縛 `fn`

```ebnf
<let_function_expr> =
    [ <pub_prefix> ] "fn" <ident> [ "=" ] <expr>

<expr> = <let_function_expr>
```

* `fn` は糖衣構文であり、`let hoist` と同じ意味を持つ。
* `<expr>` の型が関数でない場合はエラー。

### 3.4.2 スコープルール

* 変数束縛が有効なスコープは、その `let` 式が属する **もっとも狭いスコープ**。
* 関数リテラル式 / if 式 / loop 式 / match 式 / while 式に属する `<expr>` などで、
  **スコープを作らずに変数束縛式を使った場合はエラー**。

  * AST 上で「スコープノードが間にあるかどうか」で判定可能。
* `match` のパターンで導入された識別子は、対応する `case` のスコープでのみ有効。
* 関数リテラルの引数はその関数本体のスコープで有効。

---

# 4. 型システム

## 4.1 型の基本

**Type** (`/taɪp/`, 型) はすべての式に付くラベル。

* 式は必ず 1 つの値を返す。
* 複数値を返したい場合はタプル型を使う。
* 型の例：

  * `Int`, `Float`, `Bool`, `String`, `Vec[T]`
  * `enum`, `struct` 由来のユーザー定義型
  * 関数型 `(T1, T2, ..., Tn) -> R`

## 4.2 数値リテラルと暗黙の型変換

* `1` は **最初から `Int` 型**。
* `1.0` は **最初から `Float` 型**。

**Implicit conversion** (`/ɪmˈplɪs.ɪt kənˈvɝː.ʒən/`, 暗黙の型変換 [インプリシットコンバージョン]) は一切行わない。

* `Int` を要求する位置に `Float` を置いたら即エラー。
* `Dog <: Animal` のような部分型関係があっても、暗黙で `Dog` → `Animal` に変換はしない（「代入可能性」としてだけ使う）。

## 4.3 Generics（ジェネリクス）

**Generics** (`/dʒəˈne.rɪks/`, ジェネリクス) は将来導入する。

* 型変数 `T, U, ...` を持つ **多相型** をサポートする。
* オーバーロード解決時に、

  * `(Int, Int) -> Int` と `(T, T) -> T` が両方マッチする場合、
  * より具体的な単相型 `(Int, Int) -> Int` を優先する（後述の more specific ルール）。

## 4.4 enum / struct と名前解決

```ebnf
<enum_def_expr> =
    [ <pub_prefix> ] "enum" <enum_name> <scoped_list<enum_variant>>

<struct_def_expr> =
    [ <pub_prefix> ] "struct" <struct_name> <scoped_list<field>>

<enum_variant> =
    <enum_variant_name> [ "(" <type_list> ")" ]

<field> = <field_name> ":" <type>
```

* `enum` / `struct` は `namespace` 直下のスコープに現れる宣言。
* `pub` を付けることで公開。
* `enum` / `struct` 名は **型名前空間** に登録される。
* `enum` の variant 名は **値名前空間** に登録され、コンストラクタ & パターンとして使う。
* `struct` の field 名はトップレベル識別子にはならず、フィールドアクセス時にのみ使うメタ情報。

---

# 5. 関数型とオーバーロード

## 5.1 関数型と内部表現

**Function type** (`/ˈfʌŋk.ʃən taɪp/`, 関数型) は `(T1, T2, ..., Tn) -> R` の形で表現。

**Overload** (`/ˈoʊ.vɚˌloʊd/`, 多重定義 [オーバーロード]) は、同じ名前の複数の関数シグネチャの集合として表現する。

```text
f : (Int, Int) -> Int
  | (Int, Int, Int) -> Int
  | (String, String) -> String
```

内部表現イメージ：

```text
Overload {
    param_types: [Type],   // 引数型列
    result_type: Type,     // 戻り値型
    generics:    [TypeVar] // 必要なら
}

FuncType = { overloads: [Overload] }
```

## 5.2 Overload resolution のステップ

**Overload resolution** (`/ˌoʊ.vɚˈloʊd ˌrez.əˈluː.ʃən/`, オーバーロード解決) のアルゴリズムを形式化する。

### 5.2.1 Rule 0': arity で切る

**Arity** (`/ˈer.ə.t̬i/`, 引数個数 [アリティ]):

> ある関数呼び出しフレーム F で、
> 「このフレームが最終的に N 個の引数を持つ call site である」ことが
> P-style + 型推論で確定した瞬間に、
> `param_types.len() == N` のオーバーロードだけを候補に残す。

* `param_types.len() < N` → 引数が多すぎ → 不適合
* `param_types.len() > N` → 引数不足 → 不適合

これは、

* 括弧によってすでに `f e1 e2` の形が明示されている場合
* P-style のフレームが「もうこれ以上引数を取らない」と判断された場合
  どちらでも同じように適用される。

### 5.2.2 Step 1: 型によるフィルタ

call site `f e1 ... eN` に対して、候補集合を `C` とする。

各 `Oi ∈ C` について：

1. 型変数を fresh にした `Oi` を用意。
2. 各引数 `ej` の型 `type(ej)` と `Oi.param_types[j]` を unify。
3. どこか 1 つでも矛盾したら、その `Oi` は候補から除外。
4. 矛盾がなければ「`Oi` はこの call site に適合」とみなす。

### 5.2.3 Step 2: 個数による決定

* 適合候補が 0 個 → 「適合するオーバーロードがない」型エラー。
* 適合候補が 1 個 → それを採用。
* 適合候補が 2 個以上 → 次節の more specific ルールを適用。

## 5.3 more specific（より具体的）ルール

**More specific** (`/mɔːr spəˈsɪf.ɪk/`, より具体的 [モア・スペシフィック]) とは、「同じ引数に対して片方がもう片方の特殊化になっている」こと。

### 5.3.1 Rule A: 単相 vs 多相

> **Rule A（単相優先）**
> 同じ arity の 2 候補 `O1`, `O2` があり、
> 引数を当てはめた結果として
>
> * `O1` が完全に単相（型変数を含まない）
> * `O2` は型変数を含む（より一般的）
>   のとき、`O1` を `O2` より more specific とみなし優先する。

例:

```text
f : (Int, Int) -> Int
  | (T,   T  ) -> T

f 1 2  // → (Int,Int)->Int を採用
```

### 5.3.2 Rule B: Subtyping による具体性

**Subtyping** (`/ˈsʌb.taɪp.ɪŋ/`, 部分型付け [サブタイピング]) を考慮：

> **Rule B（部分型による具体性）**
> 2 候補 `O1`, `O2` のパラメータ型列が、すべての位置 j で `O1.param[j] <: O2.param[j]` かつ、
> 少なくとも 1 箇所 `k` で `O2.param[k] </: O1.param[k]` なら、
> `O1` は `O2` より more specific である。

例:

```text
feed : (Animal) -> Unit
     | (Dog)    -> Unit

Dog <: Animal
```

`feed myDog`（`myDog: Dog`）では、両候補が適合するが `(Dog)->Unit` を more specific として採用する。

### 5.3.3 最終決定

* 候補集合 `C` から、「他のどの候補からも more general とみなされない」ものを集める。
* それが

  * ちょうど 1 個 → それを採用。
  * 複数 → 「あいまいオーバーロード」エラー。

## 5.4 cast の設計

**Cast** (`/kæst/`, 型変換 [キャスト]) は、暗黙変換を提供しない代わりに **明示的な関数として提供** する。

```text
cast : (Int)   -> Float
     | (Float) -> Int
     | (Dog)   -> Animal
     | ...
```

* `cast` も普通のオーバーロード関数。
* ただし **戻り値の期待型（コンテキスト）** も使って絞り込む：

手順:

1. `cast e` のとき、まず `e` の型 `Te` を推論。
2. `param_types == [Te]` のオーバーロード候補を列挙。
3. 戻り値型について、もしコンテキストの期待型 `R_expected` があれば、

   * `result_type == R_expected` の候補だけを残す。
4. 残り 0 / 1 / 複数 の場合：

   * 0 → その変換は未定義（エラー）。
   * 1 → 採用。
   * 複数 → あいまいエラー（型注釈や別の cast を書かせる）。

---

# 6. P-style 決定アルゴリズム

## 6.1 Frame と Stack

**Frame** (`/freɪm/`, フレーム) は 1 つの関数呼び出しの途中状態：

```text
Frame {
    func_expr: Expr       // f, add, 関数リテラルなど
    overloads: [Overload] // f に対する候補集合
    args:      [Expr]     // ここまでに集まった引数
    parent:    Option<FrameId>
}
```

これを stack に積み、P-style のトークン列を左から処理する。

## 6.2 高レベルの流れ

1. ある曖昧な `<expr>` に対応する prefix 列（括弧や `{}` ごとに区切った単位）を入力とする。
2. 左から順に term を読む：

   * 関数型になりうる term を見たら、新しい Frame を push。
   * 「完成した式」（リテラル / 括弧式 / 閉じた Frame 結果など）を見たら、

     * スタックトップの Frame に引数として渡す。
3. 各 Frame では、引数が 1 つ増えるたびに

   * Overload 候補に対して部分的な型チェックを行い、
   * あり得ない候補をその場で削除していく。
4. 「これ以上この Frame に引数が渡らない」と判定できた瞬間に：

   * `args.len()` を arity として Rule 0' を適用し、
   * その Frame を確定した call site として閉じる。
5. 閉じた Frame の結果は 1 つの `Expr` として親 Frame に渡される。
6. 最終的に stack が空で、ちょうど 1 つの AST が得られれば成功。

### 6.2.1 「これ以上引数が渡らない」条件

例えば:

* 次のトークンが「別の関数」ではなく、上位レベルの式の区切り（`;`, `}`, ファイル末尾など）のとき。
* その Frame に残っているすべての Overload 候補が、これ以上引数を取る arity を持たないと判定できたとき。

などを組み合わせて判断する。

## 6.3 具体例: `add 1 add add 2 3 4`

ここでは `add : (Int, Int) -> Int` のみがあると仮定する。

トークン列:

```text
[ add0, 1, add2, add3, 2, 3, 4 ]
```

ざっくりの処理:

1. `add0` → Frame F1 を作成
2. `1`    → F1.args = [1]
3. `add2` → Frame F2（parent = F1）
4. `add3` → Frame F3（parent = F2）
5. `2`    → F3.args = [2]
6. `3`    → F3.args = [2,3]

   * `add` は 2 引数関数なので F3 はこれ以上引数を取らない → arity=2
   * Rule 0' により arity=2 の Overload だけ残す（ここでは 1 つだけ）。
   * F3 を閉じて `Expr(add 2 3)` を F2 へ渡す。
7. F2.args = [`add 2 3`]
8. `4`    → F2.args = [`add 2 3`, 4]

   * F2 も 2 引数で満杯 → F2 を閉じて `Expr(add (add 2 3) 4)` を F1 へ渡す。
9. F1.args = [1, `add (add 2 3) 4`]

   * F1 も 2 引数で満杯 → F1 を閉じる → `add 1 (add (add 2 3) 4)`

こうして、P-style の曖昧な列から一意の木構造が得られる。

3 引数版 `add` が存在する場合も、各 Frame を閉じた時点の `args.len()` で arity が決まるため、
2 引数で閉じた call site には 2 引数版だけが候補として残る。

---

# 7. 組み込み関数・標準ライブラリ・namespace

## 7.1 `@` prefix による組み込み

**Intrinsic** (`/ɪnˈtrɪn.zɪk/`, 組み込み関数 [イントリンシック]) を、特別な prefix `@` を持つ識別子として表す:

* 例: `@i32.add`, `@f64.mul`, `@memory.grow`, `@wasi.fd_write` など。

ルール:

* `let @hoge = ...` や `fn @hoge = ...` のように、`@` で始まる名前を **定義する** ことは禁止。
* 既存の `@...` 組み込みを **参照・呼び出す** のは許可（ただし主に標準ライブラリ実装向け）。
* Lexer レベルでは `<builtin_ident>` として通常の `<ident>` と区別する想定。

## 7.2 namespace と `pub`

**Namespace** (`/ˈneɪm.speɪs/`, 名前空間 [ネームスペース]) は次の構文：

```ebnf
<namespace_expr> ::= [ <pub_prefix> ] "namespace" <namespace_name> <scoped_expr>
<pub_prefix>     = "pub"
```

* `<namespace_name>` は通常の識別子。
* `namespace` 直下の `scoped_expr` 内では `pub` prefix を使える。
* さらにネストされた `scoped_expr` では `pub` を使えない。
* `namespace` 自体も `pub` を付けることで公開・非公開を制御。
* ネストした `namespace` はデフォルトで非公開であり、外側から参照するには `pub namespace` または `pub use` が必要。

### 7.2.1 use 式

```ebnf
<use_expr> =
    [ <pub_prefix> ] "use" <use_tree>

<use_tree> =
      <use_path> [ "as" <ident> ]
    | <use_path> "::" "*"

<use_path> =
    <ident> { "::" <ident> }
```

* `use ns1::ns2::func1;` → そのスコープ内で `func1` を使える。
* `use ns1::ns2::func1 as f1;` → `f1` を別名として導入。
* `use ns1::ns2::*;` → `ns1::ns2` 内の公開要素を一括導入。
* `pub use` の場合、その別名は親 namespace からも参照可能（再公開）。

## 7.3 標準ライブラリ構造と `.nepl` ファイル

標準ライブラリは `std` namespace を根にして構成する。

### 7.3.1 構造イメージ

```nepl
pub namespace std:

    include "std/math.nepl"
    include "std/string.nepl"
    include "std/vec.nepl"

    when (istarget "wasm-core"):
        include "std/platform/wasm_core.nepl"

    when (istarget "wasi"):
        include "std/platform/wasi.nepl"
```

`std/math.nepl` 側:

```nepl
pub namespace math:

    pub fn add = |Int a, Int b|->Int
        @i32.add a b

    pub fn add = |Float a, Float b|->Float
        @f64.add a b

    ; 他の演算子も同様に定義
```

ユーザからは：

```nepl
import std

std.math.add 1 2
```

のように使用できる。

### 7.3.2 include / import

```ebnf
<include_expr> ::= "include" <file_path>
<expr>         = <include_expr>

<import_expr>  ::= "import" <import_name>
<expr>         = <import_expr>
```

* `include`: 同一プロジェクト内のファイル分割を想定。

  * パスは現在のファイルからの相対パス。
  * 型は `Unit`。
* `import`: ライブラリ用途（標準ライブラリや外部パッケージ）。

  * import 名→パス解決は JSON / CSV などの一覧ファイルに基づく。
  * 名前空間構造と似た形が望ましい。
  * 型は `Unit`。

両者とも、「その部分を他ファイルの中身で置き換えたかのように扱う」が、
実際にテキスト置換はせず、エラーメッセージの位置情報などは元ファイルを保持する。

* ファイル間の循環 include/import は禁止。依存関係は DAG。

---

# 8. マルチプラットフォーム: when と istarget

## 8.1 when 構文

コンパイル時条件分岐用に **when 構文** を導入する。

```ebnf
<when_expr> ::= "when" "(" <expr> ")" <scoped_expr>
<expr>      = <when_expr>
```

制約:

* `<expr>` は **Bool 型** でなければならない。
* `<expr>` は **コンパイル時に値が確定する式** でなければならない。

  * そうでない場合、コンパイルエラー。

意味:

* `when (cond) block` は次のように解釈される：

  1. `cond` をコンパイル時に評価する。
  2. `cond == true` の場合: `block` 内を通常どおりパース・名前解決・型チェック・コード生成。
  3. `cond == false` の場合: `block` 内は **完全に無視** され、

     * 未定義シンボルや型エラーなどは報告されない。

これにより、ターゲットごとに include するファイルや定義を切り替えられる。

## 8.2 istarget 関数

**istarget** (`/aɪsˈtɑːr.ɡɪt/`, ターゲット判定) はコンパイル時関数：

```text
istarget : (String) -> Bool
```

* 引数の `String` が現在のコンパイルターゲット名と一致すれば `true`。
* 一致しなければ `false`。

例:

* `istarget "wasm-core"` → Wasm コア用ターゲットなら `true`。
* `istarget "wasi"`      → WASI 対応ターゲットなら `true`。

`when` の条件として使うことで、プラットフォームごとに標準ライブラリ構成を切り替える。

---

# 9. エラー条件と診断

## 9.1 Overload / 型まわりのエラー

1. **No overload**

   * arity フィルタ後に候補 0。
   * または型フィルタ後に候補 0。
2. **Ambiguous overload**

   * 型フィルタ後に候補 ≥ 2。
   * more specific ルールでも 1 つに絞れない。
3. **不許可な cast / Subtyping**

   * 該当する cast オーバーロードが存在しない。

エラーメッセージには、

* 候補となったシグネチャ一覧
* 実際に渡された引数の型列
* どの位置で不一致が起きたか

を含めることを目標とする。

## 9.2 P-style 解釈不能エラー

* 入力を読み切ったとき、どこかの Frame が

  * 引数不足 / 余りで閉じてしまう
  * あるいは stack が空にならず、完結する木構造が存在しない
* この場合は「P-style の曖昧な列を一貫した AST にできなかった」としてエラーを報告する。

## 9.3 when / istarget 関連エラー

* `when` の条件式が Bool 型でない → 型エラー。
* `when` の条件式がコンパイル時に値を持たない → 「コンパイル時計算不可」エラー。

---

# 10. 実装計画の概要

## 10.1 型まわりのデータ構造（Rust イメージ）

```rust
enum Type {
    Int,
    Float,
    Bool,
    String,
    Vec(Box<Type>),
    Func(FuncType),
    TypeVar(TypeVarId),
    // Enum, Struct, ...
}

struct Overload {
    param_types: Vec<Type>,
    result_type: Type,
    generics: Vec<TypeVarId>,
}

struct FuncType {
    overloads: Vec<Overload>,
}

struct TypeVar {
    id: TypeVarId,
    // union-find 的な代表・制約など
}
```

## 10.2 AST レイヤ

* `Expr` は「P-style 未確定の曖昧な AST」を表す。

  * 例: `Expr::PrefixSeq(Vec<PrefixTerm>)`。
* 型推論後には「完全に確定した AST」`ExprTyped` を構築。

  * 例: `Call { func: Box<ExprTyped>, args: Vec<ExprTyped>, ty: Type }`。

## 10.3 型推論 + P-style 決定の入口

1. 構文解析済み `Expr` を入力として受け取る。
2. その中の「P-style シーケンス」を抽出。
3. 各シーケンスごとに:

   * Frame スタックを初期化。
   * 左から term を処理。
   * Frame を閉じるたびに Overload resolution を適用し `ExprTyped` を構築。
4. 全体の制約を unify して型を確定。

## 10.4 Frame 処理の疑似コード

```text
process_prefix_terms(terms: &[Term]) -> ExprTyped {
    let mut stack: Vec<Frame> = Vec::new();

    for term in terms {
        match classify_term(term) {
            TermKind::Function(func_expr, func_type) => {
                let overloads = extract_overloads(func_type);
                stack.push(Frame::new(func_expr, overloads));
            }
            TermKind::Value(expr_typed) => {
                push_arg_to_top_frame(&mut stack, expr_typed)?;
                try_close_frames_if_possible(&mut stack)?;
            }
        }
    }

    if stack.len() != 1 || !stack[0].is_closed() {
        return Err(TypeError::UnclosedFrames);
    }

    stack.pop().unwrap().into_expr_typed()
}
```

`push_arg_to_top_frame`:

* `frame.args.push(expr)`
* `update_overload_candidates_by_type(frame, expr.ty)`
* 必要なら「これ以上引数を取れない」フラグを立てる。

`try_close_frames_if_possible`:

* トップフレームが閉じられる条件を検査。
* 閉じられるなら:

  * `arity = frame.args.len()` を確定。
  * `filter_overloads_by_arity(frame, arity)`（Rule 0'）。
  * `resolve_overload(frame)` で 1 つに決めて `ExprTyped::Call` を作る。
  * 親フレームにその結果を渡し、再帰的に親も閉じられるかチェック。

## 10.5 テスト戦略（ざっくり）

* 単純な P-style: `add 1 2`, `add 1 add 2 3` など。
* ネストした P-style: `add 1 add add 2 3 4`。
* 括弧あり: `add 1 (add 2 3)`。
* ユーザ例: `f 1 (f 2 3)`, `g (g 1 2 3)` など。
* 型混在 + オーバーロード: `h : (Int,Int)->Int | (String,String)->String` など。
* Generics + 単相: `f : (Int,Int)->Int | (T,T)->T` + `f 1 2`。
* Subtyping: `feed : (Animal)->Unit | (Dog)->Unit`。
* cast: `cast : (Int)->Float | (Float)->Int` など。

---

# 11. 今後のドキュメント分割方針（メモ）

将来的には `doc/` 以下を次のように分割していく計画とする（実際の `doc_plan.txt` は処理系実装後に整備）。

* `doc/lang/` — ユーザ向け Language Reference
* `doc/compiler/` — 実装者向け仕様（型推論・P-style アルゴリズムなど）
* `doc/stdlib/` — 標準ライブラリ API & プラットフォーム差分

現時点では、本 `starting_detail.md` を「仕様の起点」とし、
実装が進み次第ここから分割・リファクタリングしていく。
