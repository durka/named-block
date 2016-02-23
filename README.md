What it is
==========

This is a small Rust crate that provides a new control-flow primitive by means of a horrible macro.

RFC 243 (the `?`/`catch` RFC) [proposed][link] a feature called "early exit from any block". It generalizes `break` to take an expression, as well as a lifetime, and to work inside all `{}` blocks, not just loops. `break LIFE EXPR` breaks out of the block/loop identified by the lifetime, and returns the given expression from the loop. Of course the expression must have the same type as the value that the block normally returns when it ends.

[link]: https://github.com/rust-lang/rfcs/blob/master/text/0243-trait-based-exception-handling.md#early-exit-from-any-block

We can specify the desired feature by a source-to-source transformation (I doubt this is how it would be done if the feature were added to the language, but it does show that no new language features or control-flow primitives are truly required):

Input:

```rust
let x = 'a: {
    break 'a 0; // *
    1           // **
};
```

Output (asterisks show corresponding lines):

```rust
let x = {
    let ret;
    'a: loop {
        ret = {
            ret = 0;  // *
            break 'a; // *

            1         // **
        };
        break 'a;
    }
    ret
};
```

Well, that was tedious to write (and read). Let's not do that again.

```rust
#[macro_use] extern crate named_block;
#[macro_use] extern crate static_cond; // not needed on nightly

let x = block!('a: {
    break 'a 0;
    1
});
```

Fixed!

How to use it
=============

First, add "named-block" as a dependency in `Cargo.toml`. Then, add `#[macro_use] extern crate named_block;` at the top of your crate root.

If you are on nightly Rust, you can enable the "nightly" Cargo feature and skip the second step. Otherwise, you need to add "static-cond" in `Cargo.toml` and `#[macro_use] extern crate static_cond;` as well. (Check this crate's `Cargo.toml` to see which version of "static-cond" to use.)

How it works
============

The `block!` macro uses (a lot of) recursion to walk through your code and perform the source-to-source translation described above. The `ret` variable is gensymmed using hygiene and cannot collide with other variable names or even nested calls to `block!`. Neat macro tricks include using a "parsing stack" to descend into token trees, and generating new macros on the fly to do comparisons. See the commented macro source for more details.

Limitations
===========

- The macro recurses. A lot. This means it will slow down compilation proportional to the length of the code in the block. You may need to increase the recursion limit (stick `#![recursion_limit = "1000"]` at the crate root, playing with the number as necessary).
- `break LIFE EXPR` will be transformed nearly anywhere it appears.
    - Even if it's within the call to another macro, like `block!('a: { foo!(break 'a 42) })`. In principle, `foo!` could be intending to transform the syntax in some other way, and `block!` will screw it up. But it seems more likely that you _do_ want the code in macro calls to be transformed.
    - Even if it's inside a closure. This is the one that could cause problems, in rare cases. If (a) you have a closure inside a `block!` call, and (b) there is a `block!` call inside the closure, and (c) the block labels are the same... then you will get some screwy error messages and/or behavior.
    - The macro _is_ smart enough to ignore items. So blocks within local `fn`s, `impl`s, etc are safe. This should speed up parsing a bit too -- as soon as the macro sees e.g. the keyword `impl` it can skip an entire item without copying over every token or descending into token trees.
    - For closures, strange macros or other undiscovered bugs in the macro, there is a special escape hatch in the form of an attribute. Any token tree annotated with `#[block(ignore)]` will be ignored by the macro (this does not require `#![feature(stmt_expr_attributes)]` because the attribute is parsed by the macro itself).
    
        Example:

        ```rust
        block!('a: {
            enum Foo { Bar(i32) }
            let closure = #[block(ignore)] {
                move |Foo::Bar(x): Foo| -> i32 {
                    x + block!('a: {
                        break 'a 41;
                    })
                }
            };

            closure(Foo::Bar(1))
        });
        ```

        This block evaluates to `42`.

- Bare `break`/`continue` statements (lacking a specific lifetime) are not allowed within `block!` calls. This is because the macro expansion itself generates a hidden loop, so the results of these statements will be confusing and unintended (type errors, infinite loops, etc). For the same reason, you can't `continue 'a` where `'a` is the label given to `block!`. The macro will catch all of these cases during expansion and produce a compile error.

