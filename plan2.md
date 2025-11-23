# Neknaj Expression Prefix Language - General-purpose 1 実装方針

> この文書は、既存の実装方針 `plan.md` と現行仕様書 `starting_detail.md` をベースに、
> ここまでの議論内容（`while`・型付き `loop`・`mut` 引数・純粋関数 `*>`・`set` の制約・`Never` 型など）を反映した最新版の実装方針です。 

---

# 1. 概要

NEPL (Neknaj Expression Prefix Language) は、**Prefix notation** (`/ˈpriː.fɪks noʊˈteɪ.ʃən/`, 前置記法 [プレフィックス記法]) をベースにした式指向言語です。

* 構文の基本は「演算子 / 関数 + その後ろに引数列」という **P-style**（Polish-style / Prefix-style）。
* `if` / `loop` / `while` / `match` / `block` / `let` / `include` / `namespace` など、ほぼすべてが式。
* 型システムは

  * 数値型として `i32`, `i64`, `f32`, `f64`
  * 制御フロー用に **Never type** (`/ˈnɛv.ɚ taɪp/`, 決して値を持たない型 [ネバー型])
  * 関数型 `(T1, ..., Tn) -> R` と `(T1, ..., Tn) *> R`（純粋関数）
    を持つ。

処理系は Rust で実装し、`core` は `no_std` で WebAssembly を主ターゲットとする。

---

# 2. 基本的な記法と P-style

## 2.1 前置記法と曖昧な式

```ebnf
<expr> = <prefix> { <expr> }
```

* `<prefix>` は「関数 / 演算子 / 名前など」。
* `<expr>` は P-style の列であり、**構文解析時点では木構造を確定しない**。
* `()` によるグルーピング：

```ebnf
<parened_expr> = "(" <expr> ")"
<expr>         = <parened_expr>
```

型推論フェーズで、P-style の列から最終的な呼び出し構造を決定する。

## 2.2 演算子（算術 / 文字列 / ベクタ）

演算子はすべて「標準ライブラリの関数」として提供される。

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

コンパイラはこれらを単なる名前として扱い、実装は `std` ライブラリ側に任せる。

## 2.3 パイプ演算子 `>`

**Pipe operator** (`/paɪp ˈɑː.pəˌreɪ.t̬ɚ/`, パイプ演算子 [パイプ演算子]) は糖衣構文。

```ebnf
<expr>       = <pipe_chain>

<pipe_chain> = <pipe_term> { ">" <pipe_term> }    // 左結合
<pipe_term>  = <expr_without_pipe>
```

* `A > B` は「`B` に `A` を 1 番目の引数として渡す」。
* `A > F`        → `F A`
* `A > F x y`    → `F A x y`
* `A > B > C`    → `(A > B) > C`（左結合）

---

# 3. 関数リテラル・呼び出しと純粋性

## 3.1 関数リテラル

**Function literal** (`/ˈfʌŋk.ʃən ˈlɪt̬.ɚ.əl/`, 関数リテラル [ファンクションリテラル]) の構文：

```ebnf
<func_literal_expr> =
    "|" <func_literal_args> "|" ( "->" | "*>" ) <type> <expr>

<func_literal_args> =
    { <func_literal_arg> [ "," ] }

<func_literal_arg>  =
    <type> [ "mut" ] <ident>

<expr> = <func_literal_expr>
```

* 一般形：
  `|i32 a, f64 mut b|->i32  ...`
  `|i32 x|*>i32           ...`
* 「型 → 変数名」の順で書く。
* `mut` が付いた引数は「**mutable 引数**」であり、**非純粋関数（`->`）でのみ使用可能**（後述）。

### 3.1.1 純粋関数と非純粋関数

* `->` : **Impure function** (`/ɪmˈpjʊr ˈfʌŋk.ʃən/`, 非純粋関数)
* `*>` : **Pure function** (`/pjʊr ˈfʌŋk.ʃən/`, 純粋関数)

純粋関数 `*>` に対する制約：

1. 引数リスト内に `mut` を含めてはならない。
   → `|i32 mut x|*>i32 ...` はコンパイルエラー。
2. 本体内で `set` できるのは **その関数内で `let mut` されたローカル変数** のみ。
3. 本体内から呼び出せる関数は、**他の純粋関数（`*>`）のみ**。
   非純粋関数 `->` を呼び出すとコンパイルエラー。

非純粋関数 `->` では：

* `mut` 引数を使える。
* `set` によって

  * `mut` 引数
  * 外側スコープの `let mut` 変数
    を更新できる（`set` のルールに従う）。
* 純粋関数 `*>` も非純粋関数 `->` も両方呼び出せる。

## 3.2 関数呼び出し式

```ebnf
<func_call_expr> = <expr> { <expr> }
<expr>           = <func_call_expr>
```

* 先頭 `<expr>` が関数値（リテラル / 変数 / `if` / `match` など）。
* 後続の `<expr>` が引数列。
* 関数型は `(T1, ..., Tn) -> R` または `(T1, ..., Tn) *> R`（後述）。

---

# 4. 制御構造: if / loop / while / match

## 4.1 if 式

```ebnf
<if_expr>   =
    "if" <expr> ["then"] <expr>
    { "elseif" <expr> ["then"] <expr> }
    "else" <expr>

<expr> = <if_expr>
```

* 条件式の型は `Bool`。
* 各分岐の型を統合し、`Never` を考慮して if 全体の型を決定する（後述 7.3）。

## 4.2 while 式

**While expression** (`/waɪl ɪkˈsprɛʃən/`, while 式 [ワイル式])：

```ebnf
<while_expr> =
    "while" <expr> <scoped_expr>

<expr> = <while_expr>
```

* 条件 `<expr>` の型は `Bool` でなければならない。
* 本体 `<scoped_expr>` 内で何をしても、**`while` 式全体の型は常に `Unit`**。
* 本体内では

  * `break`（値なし）
  * `continue`
  * `return expr`
    を使えるが、`break expr`（値付き）は禁止。

## 4.3 loop 式（型付きループ）

**Loop expression** (`/luːp ɪkˈsprɛʃən/`, loop 式 [ループ式])：

```ebnf
<loop_expr> =
    "loop" <scoped_expr>

<expr> = <loop_expr>
```

### 4.3.1 break / continue の構文と型

**Break expression** (`/breɪk ɪkˈsprɛʃən/`, break 式 [ブレイク式])
**Continue expression** (`/kənˈtɪn.juː ɪkˈsprɛʃən/`, continue 式 [コンティニュー式])：

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

* `break` / `break expr` / `continue` いずれも **式としての型は `Never`**。
  （ただし `break expr` の `<expr>` 自体には普通の型 `T` が付く）
* `Never` は「**絶対に通常の制御フローに戻らない式**」を表す型（後述 7.2）。

### 4.3.2 loop の型規則（型付きループ）

`loop` 本体の中の `break` を見て、`loop` 式全体の型を決める：

1. **値付き `break expr` が 1 つもない場合**
   → `loop` 式の型は `Unit`。
2. **値付き `break expr` が 1 つ以上ある場合**
   → それらに現れる `expr` の型はすべて同一 `T` でなければならない。
   → `loop` 式の型は `T`。
3. `loop` の型が `T != Unit` の場合は、**値なし `break` を含めてはならない**。
   （値なし `break` は「`Unit` を返すループ」でのみ許される。）

例：

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

## 4.4 match 式とパターン

### 4.4.1 構文

**Pattern** (`/ˈpæt.ɚn/`, パターン [パターン]) を使う **Match expression**：

```ebnf
<match_case> =
    "case" <pattern> "=>" <expr>

<match_expr> =
    "match" <expr> <scoped_list<match_case>>

<expr> =
      <match_expr>
    | ...
```

> ※ 以前のような `match cmd with | Quit -> ...` という ML 風ではなく、
> NEPL では必ず `case` と `=>` を用いる。

例：

```nepl
match cmd:
    case Quit           => break
    case Step(n)        => do_step n
    case Other          => continue
```

### 4.4.2 パターンの種類

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

Enum 用：

```ebnf
<enum_variant_pattern> =
      <enum_variant_name> "(" <pattern_list> ")"
    | <enum_variant_name>

<pattern_list> = <pattern> ("," <pattern>)*
```

Struct 用：

```ebnf
<struct_pattern> =
    <struct_name> "{" <field_pattern_list> "}"

<field_pattern_list> = <field_pattern> ("," <field_pattern>)*
<field_pattern>      = <field_name> ":" <pattern>
```

### 4.4.3 match のスコープ

* `match expr:` の後ろ全体が新しいスコープ。
* 各 `case` の `<pattern>` 内で導入された識別子は、**その case の本体 `<expr>` のみで有効**。

---

# 5. スコープ・ブロック・スコープ付きリスト

## 5.1 スコープ構文

```ebnf
<scoped_expr> =
      "{" <expr> "}"
    | ( ":" <expr> with off-side rule )

<expr> = <scoped_expr>
```

* `{ ... }` : C 風ブロック。中身が 1 スコープ。
* `: ...` : Python 風。`:` 行よりインデントが深い行がスコープ内。

## 5.2 ブロック式

```ebnf
<block_expr> =
    { <expr> ";" }+ <expr>

<expr> = <block_expr>
```

* 複数の式を `;` でつなぎ、最後の式の値を返す。
* 途中の式の型が `Unit` 以外なら警告（無視されている計算）。

## 5.3 スコープ付きリスト `<scoped_list<Item>>`

`match` / `enum` / `struct` の共通構文：

```ebnf
<scoped_list<Item>> =
      "{" <item_list<Item>> "}"
    | ( ":" <item_list<Item>> with off-side rule )

<item_list<Item>> =
    { Item ("," | ";") }+
```

* `Item` には `<match_case>` / `<enum_variant>` / `<field>` など。
* カンマとセミコロンどちらでも区切れる。

---

# 6. 変数束縛 `let` と代入 `set`

## 6.1 変数束縛 `let`

```ebnf
<let_suffix> = "mut" | "hoist"
<pub_prefix> = "pub"

<let_expr> =
    [ <pub_prefix> ] "let" [ <let_suffix> ] <ident> [ "=" ] <expr>

<expr> = <let_expr>
```

ルール（再掲＋補足）：

* デフォルトで immutable。
* `let mut x = expr` で mutable 変数束縛。
* `let hoist x = expr`

  * 同一スコープ内で前方参照可能（巻き上げ）。
  * `mut` と共存不可（定数専用）。
* `namespace` 直下では `pub let x = expr` で公開定数（`mut` と共存不可）。
* `_` 始まりの識別子は「未使用でも警告なし」。
* `_` 単独は「どこにも束縛しない」。

スコープ：

* `let` の有効範囲は、その `let` 式が現れる最も狭いスコープ。
* 関数リテラル / if / loop / while / match 本体など、**スコープを作るべき場所にスコープがないのに `let` があるとエラー**。

## 6.2 代入式 `set` と assignable

**Assignable expression** (`/əˈsaɪ.nə.bəl ɪkˈsprɛʃən/`, 代入可能式 [アサイナブル式]) を導入し、`set` の左辺に使う。

```ebnf
<assignable> =
      <ident>
    | <field_expr>

<field_expr> =
    <expr> "." <ident>

<set_expr> =
    "set" <assignable> <expr>

<expr> = <set_expr>
```

### 6.2.1 一般ルール

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

### 6.2.2 純粋関数内での `set`

純粋関数 `*>` の本体では、さらに制約がかかる：

* `set` できるのは **その関数のローカルスコープで `let mut` された変数**（およびそのフィールド）のみ。
* 外側スコープの変数（クロージャキャプチャなど）を `set` するのは禁止。

例：

```nepl
let mut x = 0

let f = |i32 y|*>Unit:
    let mut z = y
    set z (z + 1)     ; OK: ローカル let mut
    set x (x + 1)     ; エラー: 外側スコープの変数に set

f 10
```

---

# 7. 型システム：組み込み型・関数型・Never 型

## 7.1 組み込み型

**Type** (`/taɪp/`, 型 [タイプ]) の基本集合：

* `i32`, `i64` : 32/64-bit signed integer
* `f32`, `f64` : 32/64-bit IEEE 754 floating-point
* **Bool** (`/buːl/`, 真偽値 [ブール型])
* **Unit** (`/ˈjuː.nɪt/`, 単位型 [ユニット型])
* **Never type** (`/ˈnɛv.ɚ taɪp/`, 決して値を返さない型 [ネバー型])
* ユーザー定義型: `enum` / `struct` など

数値リテラルのデフォルト：

* 整数リテラル `1` → `i32`
* 小数リテラル `1.0` → `f64`

**Implicit conversion** (`/ɪmˈplɪs.ɪt kənˈvɝː.ʒən/`, 暗黙の型変換 [インプリシットコンバージョン]) は行わない。

## 7.2 Never 型と制御フロー

**Never type** は「値を一切持たない底型」であり、以下の式に付く：

* `return expr`
* `break`
* `break expr`
* `continue`

これらは「必ずその位置から通常の実行には戻らない」ので、式としての型を `Never` とする。

性質：

* `Never` はすべての型 `T` の **subtype** (`/ˈsʌb.taɪp/`, 部分型 [サブタイプ]) として扱う
  → `Never` を `T` として見なすことが許される。

## 7.3 if / match の型付けと Never

**If expression** と **Match expression** の型は、各分岐の型の「共通 supertype」を取り、`Never` を底型として扱うことで決定する。

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

`Never` を底型として扱うことで、`match` 全体の型を `Unit` と決定できる。

## 7.4 関数型と純粋性

**Function type** (`/ˈfʌŋk.ʃən taɪp/`, 関数型 [ファンクションタイプ])：

```ebnf
<func_type> =
    "(" [ <type> { "," <type> } ] ")" ( "->" | "*>" ) <type>
```

* `(T1, ..., Tn) -> R` : 非純粋関数型
* `(T1, ..., Tn) *> R` : 純粋関数型

純粋関数から呼び出せるのは `*>` 型の関数のみ、という制約を型レベルで表現できる。

---

# 8. 関数のオーバーロードと P-style 決定

（この節は元の `plan.md` / `starting_detail.md` の内容を基本的に踏襲し、
型や関数型の拡張に合わせて微修正する。） 

## 8.1 Overload の内部表現

**Overload** (`/ˈoʊ.vɚˌloʊd/`, 多重定義 [オーバーロード]) は、同名関数の複数シグネチャの集合。

```text
Overload {
    param_types: [Type],
    result_type: Type,
    arrow_kind:  ArrowKind, // -> or *>
    generics:    [TypeVar],
}
```

`FuncType` は `Overload` のリスト。

## 8.2 Overload resolution の手順（概要）

1. **Arity（引数個数）で候補を絞る**。
2. 各候補に対し、引数型との unify を試みる。
3. 適合候補が 0 → エラー。
4. 適合候補が 1 → 決定。
5. 適合候補が複数 → **more specific**（単相優先 / 部分型がより具体的）ルールで 1 つに絞る。
   それでも 1 つに決まらなければ「あいまいオーバーロード」エラー。

`*>` / `->` が異なるオーバーロード同士は、**arrow_kind も含めて別物**として扱う。

## 8.3 P-style 決定アルゴリズム

P-style の曖昧な列を **Frame**（1 つの呼び出しの途中状態）のスタックとして処理する。

* 新しい関数候補を見たら Frame を push。
* 値（完成した式）が来たらトップフレームに引数として追加。
* 引数が増えるたびに、その場でオーバーロード候補をフィルタ。
* 「これ以上引数が増えない」と判定できたところで Frame を閉じ、
  確定した `Call` ノードとして親 Frame に渡す。

---

# 9. namespace / include / import / use / enum / struct

このあたりは元の `plan.md` の内容をほぼそのまま利用できる。
変更点は「型名に `i32` などを使う」程度。

## 9.1 include / import / namespace / use

* `include` / `import` はファイル分割とライブラリ利用のための仕組み。
* `namespace` は名前空間を作り、`pub` による公開制御を行う。
* `use` はスコープ内に別名を導入し、`pub use` で再公開も可能。

## 9.2 enum / struct

`enum` / `struct` は `namespace` 直下の宣言で、`pub` による公開制御を持つ。

* `enum` 名 / `struct` 名は型名前空間に登録。
* `enum` の variant 名は値名前空間に入り、コンストラクタ・パターンとして使える。
* `struct` の field 名はトップレベルには出さず、`p.x` 等のフィールドアクセス時にのみ解決。

---

# 10. マルチプラットフォーム: when / istarget

**when expression** と `istarget` の仕様は基本的に既存のまま。

* `when (cond) block` はコンパイル時条件分岐。
* `cond` は `Bool` かつコンパイル時計算可能でなければならない。
* `istarget : (String) -> Bool` を使ってターゲット判定を行い、条件に応じて `include` するファイル等を切り替える。

---

# 11. 実装上のメモ

* 構文解析は

  * P-style の列
  * スコープ構造（`{}` / `:`）
  * `include` / `import` / `namespace` / `use` / `enum` / `struct`
    を処理し、**まだ木構造が曖昧な AST** を作る。
* 名前解決フェーズで

  * `let hoist` / `fn` / `enum` / `struct` の前方参照
  * `namespace` と `use` によるパス解決
  * `pub` による公開可視性
    を処理する。
* 型推論フェーズで

  * P-style の Frame スタックによる木構造決定
  * Overload resolution
  * `*>` / `->` の純粋性制約
  * `Never` を含む制御フローの整合
    を一気にチェックする。
* エラーは可能な限り回復して、1 回のコンパイルで多くの問題を報告できるようにする。