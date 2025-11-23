# typing_detail.md — 型システム & オーバーロード仕様 / 実装計画

この文書は **Neknaj Expression Prefix Language** の
型システム・オーバーロード・P-style 引数決定の詳細仕様と、その実装計画をまとめたものです。

---

# 1. 位置づけと前提

## 1.1 処理系パイプライン

処理系は次の段階で動く：

1. **Lexical analysis** (`/ˈlek.sɪ.kəl əˈnæl.ə.sɪs/`, 字句解析)
2. **Parsing** (`/ˈpɑːr.sɪŋ/`, 構文解析)
3. **Name resolution & Type inference** (`/neɪm ˌrez.əˈluː.ʃən/`, 名前解決; `/taɪp ˈɪn.fər.əns/`, 型推論)
4. その他チェック

本書が主に対象とするのは 3. の段階です。

## 1.2 P-style（Prefix 記法）と曖昧な構文木

* 式の基本は **Prefix notation**（`/ˈpriː.fɪks noʊˈteɪ.ʃən/`, 前置記法 [プレフィックスきほう]）：

  * `<expr> = <prefix> { <expr> }`
* 構文解析では

  * 括弧 `()`、スコープ `{}` / `:`、include/import などは解決するが、
  * **「どの `<prefix>` が何個の引数を取るか」「関数／値の区別」などは決定しない**。

→ 型推論段階で、**P-style の木構造とオーバーロードを同時に確定させる**。

---

# 2. 型システムの基本方針

## 2.1 型の基本

* すべての式 `<expr>` は必ず **Type**（`/taɪp/`, 型）を持つ。
* 複数値の返却はしない（タプルを使う）。
* 型には少なくとも以下を含む：

  * `Int`, `Float`, `Bool`, `String`, `Vec[T]`, `enum`, `struct`
  * **Function type**（`/ˈfʌŋk.ʃən taɪp/`, 関数型）: `(T1, T2, ..., Tn) -> R`

## 2.2 数値リテラルと暗黙変換

* `1` は **最初から `Int` 型**。
* `1.0` は **最初から `Float` 型**。
* **Implicit conversion**（`/ɪmˈplɪs.ɪt kənˈvɝː.ʒən/`, 暗黙の型変換 [インプリシットコンバージョン]）は行わない。

  * `Int` が要求されているところに `Float` を置いたら **型エラー**。
  * `Dog <: Animal` のような部分型も、暗黙変換はしない（代入可能性だけに使う）。

## 2.3 Generics（ジェネリクス）

* **Generics**（`/dʒəˈne.rɪks/`, ジェネリクス）は将来導入する。
* 型変数 `T, U, ...` を含む型（多相型）を扱う。
* オーバーロード解決時には、

  * 単相型 `(Int,Int)->Int` と多相型 `(T,T)->T` が両方マッチする場合、
  * **より具体的（more specific）な型を優先する**（後述）。

---

# 3. 関数とオーバーロード表現

## 3.1 関数リテラルと呼び出し

* 関数リテラル：

  ```ebnf
  <func_literal_expr> = "|" <func_literal_args> "|" ( "->" | "*>" ) <type> <expr>
  <func_literal_args> = { <func_literal_arg> [ "," ] }
  <func_literal_arg>  = <type> <ident>
  <expr>              = <func_literal_expr>
  ```

* 関数呼び出し：

  ```ebnf
  <func_call_expr> = <expr> { <expr> }
  <expr>           = <func_call_expr>
  ```

1つ目の `<expr>` が関数、以降が引数。

## 3.2 Overload（多重定義）の表現

**Overload**（`/ˈoʊ.vɚˌloʊd/`, 多重定義 [オーバーロード]）は、**Union type**で表現する：

```text
f : (Int, Int) -> Int
  | (Int, Int, Int) -> Int
  | (String, String) -> String
```

内部表現イメージ：

```text
Overload {
    param_types: [Type],   // 引数型の列
    result_type: Type,     // 戻り値型
    generics:    [TypeVar] // 必要なら
}

FunctionType = Union<Overload>
```

---

# 4. Overload resolution の仕様

ここでは

* **呼び出し位置（call site）ごとに**
* どのオーバーロードを選ぶか

を形式的に定める。

## 4.1 ステップ 0' — arity で切る（修正版）

**Arity**（`/ˈer.ə.t̬i/`, 引数個数 [アリティ]）について：

> **Rule 0'（局所 arity フィルタ）**
> ある関数呼び出しフレーム F で、
> 「このフレームが最終的に N 個の引数を持つ call site である」ことが
> P-style + 型推論の処理で確定したとき、
> その瞬間に F に対して
> `param_types.len() == N` のオーバーロードだけを候補として残す。

* `param_types.len() < N` → 「引数が多すぎる」 → 不適合
* `param_types.len() > N` → 「引数不足」 → 不適合

**重要**：
「最初から `f e1 ... eN` が分かっている」場合（括弧などで区切られた call site）でも、
P-style のスタックフレームが閉じた瞬間でも、**同じ規則**を適用する。

## 4.2 ステップ1 — 型によるフィルタ

call site `f e1 ... eN` に対して、候補オーバーロード集合を `C` とする。

各 `Oi ∈ C` について：

1. 型変数を fresh にした `Oi` を用意する。
2. 各引数 `ej` の型 `type(ej)` と `Oi.param_types[j]` を unify する。
3. どこか1つでも矛盾したら、その `Oi` は候補から除外。
4. 矛盾せずに unify できたら、「`Oi` は `f e1 ... eN` に適合」とみなす。

これにより「引数型で合わないオーバーロード」は全て落ちる。

## 4.3 ステップ2 — 個数による決定

* 適合候補が **0 個** → 「適合するオーバーロードがない」型エラー。
* 適合候補が **1 個** → それを選択。
* 適合候補が **2 個以上** → 「具体性ルール」で決める（次節）。

## 4.4 ステップ3 — more specific（より具体的）ルール

**More specific**（`/mɔːr spəˈsɪf.ɪk/`, より具体的 [モア・スペシフィック]） の判定：

### 4.4.1 単相 vs 多相（Generics）

> **Rule A（単相優先）**
> 同じ arity の 2 つの候補 `O1`, `O2` が、
> 引数を当てはめた結果として
>
> * `O1` は型変数を含まない（完全に単相）
> * `O2` はまだ型変数を含む（より一般的）
>
> なら、`O1` を **more specific** とみなし、優先する。

例：

```text
f : (Int, Int) -> Int
  | (T,   T  ) -> T
```

`f 1 2` では：

* `(Int,Int)->Int` : 単相
* `(T,T)->T` : `T=Int` で適合するが、元は多相

→ `(Int,Int)->Int` を採用。

### 4.4.2 Subtyping による具体性

**Subtyping**（`/ˈsʌb.taɪp.ɪŋ/`, 部分型付け [サブタイピング]）を考慮したルール：

> **Rule B（部分型による具体性）**
> 2 つの候補 `O1`, `O2` のパラメータ型列を比較し、
> すべての位置 j について `O1.param[j] <: O2.param[j]` であり、
> 少なくとも1箇所は `O2.param[k] </: O1.param[k]` であるなら、
> `O1` は `O2` より **more specific** である。

例：

```text
feed : (Animal) -> Unit
     | (Dog)    -> Unit
```

`Dog <: Animal` のとき、

* 引数 `Dog` に対して両方適合するが、`Dog` 版のほうがより具体的。
* 暗黙変換はしない（`Cat` を `Dog` にすることはない）が、`Dog` は `Animal` の代入可能なサブタイプ。

### 4.4.3 最終決定

候補集合 `C` に対して：

* 「他のどの候補からも『より一般的』とはみなされない」ものが

  * ちょうど1個 → それを採用。
  * 2個以上 → 「あいまいオーバーロード」エラー。

---

# 5. P-style + オーバーロード + 引数決定アルゴリズム

## 5.1 用語: Frame（フレーム）と Stack

**Frame**（`/freɪm/`, フレーム）は 1 つの関数呼び出しの途中状態を表す：

```text
Frame {
    func_expr: Expr       // f, add, 関数リテラルなど
    overloads: [Overload] // f に対する候補集合
    args: [Expr]          // ここまでに集まった引数
    parent: Option<FrameId>
}
```

これらを stack に積んで、P-style のトークン列を左から処理する。

## 5.2 高レベルの流れ

1. 入力：ある「曖昧な `<expr>`」に対応する prefix 列（括弧や `{}` で区切られた単位ごと）。
2. 左から順に term を読む：

   * 関数型になりうる `term` を見たら、新しい Frame を push。
   * 「完成した式」（リテラル、括弧で囲まれた式、閉じた Frame の結果など）を見たら、

     * スタックトップの Frame にその式を引数として渡す。
3. 各 Frame では、引数が 1 個増えるごとに

   * 候補オーバーロードに対して型チェック（部分的 unify）を行い、
   * **その段階であり得ない候補は削除**していく。
4. 「これ以上この Frame に引数が渡らない」と判断できた瞬間に、

   * その Frame の `args.len()` を arity として、**Rule 0'** を適用し
   * その Frame を「確定した call site」として閉じる。
5. 閉じた Frame の結果は 1 つの `Expr` として親 Frame に渡される。
6. 最終的にスタックが空で、1 つの AST が得られれば成功。

### 「これ以上引数が渡らない」条件（例）

* 次のトークンが「別の関数」ではなく、

  * 上位レベルの式の区切り（`;`, `}`, ファイル末尾など）
  * あるいは親 Frame が閉じるトリガ
* または、「この Frame に残っているすべてのオーバーロード候補が、
  これ以上引数を受け取る arity を持たない」とき

などを組み合わせて判定する。

## 5.3 例: `add 1 add add 2 3 4`

ここでは簡単のため `add : (Int,Int)->Int` のみを持つとする。

### トークン列

```text
[ add0, 1, add2, add3, 2, 3, 4 ]
```

### 処理のざっくりなイメージ

1. `add0` → Frame F1 作成
2. `1` → F1.args = [1]
3. `add2` → Frame F2 作成（parent = F1）
4. `add3` → Frame F3 作成（parent = F2）
5. `2` → F3.args = [2]
6. `3` → F3.args = [2,3]

   * `add` は2引数関数なので、
     F3 は「これ以上引数を取らない」ことが決定 → call site の arity = 2
   * Rule 0' により arity=2 のオーバーロードだけを残す
   * F3 を閉じて `Expr(add 2 3)` を親 F2 へ。
7. F2.args = [ `add 2 3` ]
8. `4` → F2.args = [ `add 2 3`, 4 ]

   * F2 も 2 引数で満杯 → F2 を閉じて `Expr(add (add 2 3) 4)` を親 F1 へ。
9. F1.args = [1, `add (add 2 3) 4`]

   * F1 も 2 引数 → F1 を閉じる → `add 1 (add (add 2 3) 4)`

最終構造：

```text
add 1 (add (add 2 3) 4)
```

この時点で、各 call site `add (...) (...)` に対して
「arity=2」「引数型は Int, Int」という情報が揃っているので、
前節の Overload resolution を適用すればよい。

### 3 引数版とのからみ（将来）

もし `add : (Int,Int)->Int | (Int,Int,Int)->Int` だとしても、
「フレームを閉じた時点の `args.len()`」で arity が決まるので、

* 2 引数で閉じたところは 2 引数版だけ候補に残る
* 3 引数で閉じたところは 3 引数版だけ候補に残る

という形になる。

---

# 6. Generics + Overload の例

## 6.1 単相 vs 多相

```text
f : (Int, Int) -> Int
  | (T,   T  ) -> T
```

```text
f 1 2
```

1. P-style で `f [1,2]` という call site が確定。
2. arity=2 → 両オーバーロード候補。
3. 型チェック：

   * `(Int,Int)->Int` : そのまま OK。
   * `(T,T)->T` : `T = Int` で OK。
4. どちらも適合 → Rule A（単相優先）を適用。
5. `(Int,Int)->Int` を **more specific** として採用。

## 6.2 部分型 + Generics

```text
feed : (Animal) -> Unit
     | (Dog)    -> Unit
```

`Dog <: Animal` のとき：

```text
feed myDog   // myDog : Dog
```

* 両オーバーロード候補が適合。
* Rule B（Subtyping）により `(Dog)->Unit` を more specific とみなし採用。

---

# 7. cast の設計

## 7.1 基本方針

**Cast**（`/kæst/`, 型変換 [キャスト]）は、
**暗黙変換を提供しない代わりに、明示的な関数として提供する**：

```text
cast : (Int)   -> Float
     | (Float) -> Int
     | (Dog)   -> Animal
     | ...
```

* Cast も普通の関数と同じく、オーバーロード解決の対象になる。
* ただし、**戻り値の期待型（コンテキスト）** も使って判定する。

## 7.2 解決手順

呼び出し `cast e` について：

1. `e` の型 `Te` を推論。
2. `param_types == [Te]` のオーバーロード候補を列挙。
3. 戻り値型に関して、

   * もし「コンテキストで期待される型 `R_expected`」があるなら、

     * `result_type == R_expected` の候補だけを残す。
4. 0 / 1 / 複数 の場合は、通常の Overload resolution と同じ：

   * 0 → エラー（その変換は定義されていない）
   * 1 → 採用
   * 複数 → あいまいエラー（型注釈や別の cast を書かせる）

### 例

```text
cast 1
```

* `1 : Int`
* 候補 `(Int)->Float`, `(Int)->String` があった場合、コンテキストが無いとあいまい。
* 次のような場合：

  ```text
  let x : Float = cast 1
  ```

  → `Float` が期待される戻り値型なので、`(Int)->Float` のみ適合 → 採用。

---

# 8. エラー条件のまとめ

1. **No overload**

   * arity フィルタ後に候補 0
   * または型フィルタ後に候補 0
2. **Ambiguous overload**

   * 型フィルタ後に候補 ≥ 2
   * 具体性ルールによっても 1 つに絞れない
3. **P-style 解釈不能**

   * 入力を読み切ったときに、どこかの Frame が

     * 「引数不足」または
     * 「引数余り」で閉じてしまう
   * または「完結する木構造」が存在しない
4. **Subtyping / cast で不許可な変換**

   * その型の変換オーバーロードが存在しない

エラーメッセージには、少なくとも

* 適合しなかった候補の型一覧
* 実際に渡された引数の型列
* どの位置で不一致が起きたか

を含める。

---

# 9. 実装計画（Rust ベース）

## 9.1 型まわりのデータ構造

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
    generics: Vec<TypeVarId>, // 必要なら
}

struct FuncType {
    overloads: Vec<Overload>, // Union
}
```

型推論用に：

```rust
struct TypeVar {
    id: TypeVarId,
    // union-find 的な代表 / 制約など
}
```

## 9.2 AST レイヤ

* `Expr` は「P-style 未解決の曖昧な AST」を保持：

  * `Expr::PrefixSeq(Vec<PrefixTerm>)` など。
* 型推論後に、「完全に確定した AST」を作る：

  * `ExprTyped` として、`Call { func: Box<ExprTyped>, args: Vec<ExprTyped>, ty: Type }` など。

## 9.3 型推論 + P-style 決定のエントリポイント

1. 構文解析済みの `Expr` を受け取る。
2. その中の「P-style シーケンス」を見つける。
3. 各シーケンスごとに

   * Frame スタックを初期化
   * 左から term を処理
   * Frame を閉じるたびに Overload resolution を適用し、`ExprTyped` を構築
4. 途中で型変数・制約を溜め込み、最後に unify。

## 9.4 Frame 処理の擬似コードイメージ

※ 実際の Rust コード化の前段の設計用イメージ。

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

`push_arg_to_top_frame` の中で：

* `frame.args.push(expr)`
* `update_overload_candidates_by_type(frame, expr.ty)`
* 必要なら、「もうこれ以上引数を取れない」ことをマーク

`try_close_frames_if_possible` の中で：

* 「トップフレームが閉じられる条件」をチェック
* 閉じられるなら

  * `arity = frame.args.len()` を確定
  * `filter_overloads_by_arity(frame, arity)`（Rule 0'）
  * `resolve_overload(frame)` で 1 つに決めて `ExprTyped::Call` を作る
  * 親フレームにその `ExprTyped` を渡す
  * さらに親フレームも閉じられるか再帰的に試す

## 9.5 テスト戦略

* 単純な P-style：

  * `add 1 2`, `add 1 add 2 3` など。
* ネストした P-style：

  * `add 1 add add 2 3 4` など。
* 括弧あり：

  * `add 1 (add 2 3)`
  * `f 1 (f 2 3)` / `g (g 1 2 3)` など、ユーザーが挙げた例。
* 型混在 + オーバーロード：

  * `h : (Int,Int)->Int | (String,String)->String`
  * `k : (Int,Int)->Int | (Float,Float)->Float`
* Generics + 単相：

  * `f : (Int,Int)->Int | (T,T)->T` + `f 1 2`
* Subtyping：

  * `feed : (Animal)->Unit | (Dog)->Unit`
* cast：

  * `cast : (Int)->Float | (Float)->Int` など。
