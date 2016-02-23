// TODO
// - comment macro
// - support loops?
// - eliminate the closure hole by recognizing nested calls and shadowed labels, then maintaining a whitelist?

#![cfg_attr(test, recursion_limit = "1000")]
#![cfg_attr(feature = "nightly", feature(macro_reexport))]

#[cfg(all(test, not(feature = "nightly")))]                #[macro_use] extern crate static_cond;
#[cfg(feature = "nightly")] #[macro_reexport(static_cond)] #[macro_use] extern crate static_cond;

/// Provides the "early exit from any block" control-flow primitive that was mentioned in [RFC 243][link].
///
/// If not using the "nightly" Cargo feature, you must depend on `static-cond` and put `#[macro_use] extern crate static_cond;` at the crate root.
///
/// See README.md for more details.
///
/// [link]: https://github.com/rust-lang/rfcs/blob/master/text/0243-trait-based-exception-handling.md#early-exit-from-any-block
///
/// Examples
/// ========
///
/// ```
/// # #[macro_use] extern crate named_block;
/// # #[macro_use] extern crate static_cond;
/// # fn main() {
/// let x = block!('a: {
///     break 'a 0;
///     1
/// });
/// assert_eq!(x, 0);
/// # }
/// ```
///
/// ```
/// # #[macro_use] extern crate named_block;
/// # #[macro_use] extern crate static_cond;
/// # fn main() {
/// assert_eq!(
///     42,
///     block!('a: {
///         enum Foo { Bar(i32) }
///         let closure = #[block(ignore)] {
///             move |Foo::Bar(x): Foo| -> i32 {
///                 x + block!('a: {
///                     break 'a 41;
///                 })
///             }
///         };
///     
///         closure(Foo::Bar(1))
///     })
/// );
/// # }
/// ```
#[macro_export]
macro_rules! block {
    // =======================================================================================
    // PRIVATE RULES
    // =======================================================================================
    
    // ======================================================
    // UTILITY RULES
    // ======================================================

    // utility: coerce an AST fragment with interpolated TTs into an expression
    // (see https://danielkeep.github.io/tlborm/book/blk-ast-coercion.html)
    (@as_expr $e:expr) => { $e };
    
    // utility: deliberately cause a compile error with a CamelCaseMessage
    (@error $err:ident) => {{
        struct $err;
        let _: () = $err;
    }};

    // ======================================================
    // OUTPUT STAGE
    // ======================================================
    // This is called from the scanner when everything has
    // been processed and we are ready to write out the
    // final expansion.
    
    // final output from the top level of the macro
    (@wrap $life:tt $ret:ident $out:expr) => {
        block!(@as_expr
            {
                let $ret;
                $life: loop {
                    $ret = $out;
                    break $life;
                }
                $ret
            })
    };

    // ======================================================
    // SCANNER STAGE
    // ======================================================
    // This is the meat of the macro. It transfers code from
    // the input (left of ->) to the output (right of ->)
    // while transforming break statements or triggering
    // errors as needed.

    // The next four rules handle the end of the input code -- either the macro
    // is done, or we need to pop the stack and keep walking. We can tell which
    // it is by checking the context stack. If it's empty, we can go to output.
    // Otherwise, we take the current output, surround it by the brace type,
    // and move up the context stack.
    
    // no context: we're done!
    (@scan {} $life:tt $ret:ident () -> ($($out:tt)*) ()) => {
        block!(@wrap $life $ret { $($out)* })
    };
    // pop stack and surround with {}
    (@scan {} $life:tt $ret:ident () -> ($($out:tt)*) $ctx:tt) => {
        block!(@up $life $ret { $($out)* } $ctx)
    };
    // pop stack and surround with ()
    (@scan () $life:tt $ret:ident () -> ($($out:tt)*) $ctx:tt) => {
        block!(@up $life $ret ( $($out)* ) $ctx)
    };
    // pop stack and surround with []
    (@scan [] $life:tt $ret:ident () -> ($($out:tt)*) $ctx:tt) => {
        block!(@up $life $ret [ $($out)* ] $ctx)
    };
    
    // The next nine rules are triggered when the tree walker encounters a
    // break/continue statement.

    // bare "break" and "continue" statements are errors (TODO allow bare break?)
    (@scan $paren:tt $life:tt $ret:ident (break) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret () -> ($($out)* block!(@error NoBareBreakInNamedBlock);) $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (break; $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* block!(@error NoBareBreakInNamedBlock);) $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (continue) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret () -> ($($out)* block!(@error NoBareContinueInNamedBlock);) $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (continue; $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* block!(@error NoBareContinueInNamedBlock);) $ctx)
    };
    // "break LIFETIME;" (no EXPR)
    (@scan $paren:tt $life1:tt $ret:ident (break $life2:tt; $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* break $life2;) $ctx)
    };
    // "break LIFETIME EXPR": compare the lifetimes, if they match then transform the statement, otherwise leave it alone
    (@scan $paren:tt $life1:tt $ret:ident (break $life2:tt $e:expr; $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        static_cond! {
            if $life1 == $life2 {
                block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* { $ret = $e; break $life2; }) $ctx)
            } else {
                block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* break $life2 $e;) $ctx)
            }
        }
    };
    (@scan $paren:tt $life1:tt $ret:ident (break $life2:tt $e:expr) -> ($($out:tt)*) $ctx:tt) => {
        static_cond! {
            if $life1 == $life2 {
                block!(@scan $paren $life1 $ret () -> ($($out)* { $ret = $e; break $life2 }) $ctx)
                    // TODO make sure this isn't adding too many semicolons
            } else {
                block!(@scan $paren $life1 $ret () -> ($($out)* break $life2 $e;) $ctx)
            }
        }
    };
    // "continue LIFETIME": compare the lifetimes, if they match then error, otherwise leave it alone
    (@scan $paren:tt $life1:tt $ret:ident (continue $life2:tt; $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        static_cond! {
            if $life1 == $life2 {
                block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* block!(@error NoMatchedContinueInNamedBlock);) $ctx)
            } else {
                block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* continue $life2;) $ctx)
            }
        }
    };
    (@scan $paren:tt $life1:tt $ret:ident (continue $life2:tt) -> ($($out:tt)*) $ctx:tt) => {
        static_cond! {
            if $life1 == $life2 {
                block!(@scan $paren $life1 $ret () -> ($($out)* block!(@error NoMatchedContinueInNamedBlock);) $ctx)
            } else {
                block!(@scan $paren $life1 $ret () -> ($($out)* continue $life2;) $ctx)
            }
        }
    };

    // tree walker ignores #[block(ignore)] tts, closures, and items
    
    (@scan_item $paren:tt $life:tt $ret:ident ($ignore:item $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* $ignore) $ctx)
    };
    
    // #[block(ignore)] attribute is ignored
    (@scan $paren:tt $life:tt $ret:ident (#[block(ignore)] $ignore:tt $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* $ignore) $ctx)
    };
    // other attributes pass through
    (@scan $paren:tt $life:tt $ret:ident (#[$attr:meta] $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* #[$attr]) $ctx)
    };
    // ignore items: use, extern, static, const, unsafe trait/impl/fn, fn, mod, type, enum, trait, impl, struct
    (@scan $paren:tt $life:tt $ret:ident (pub $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (pub $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (use $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (use $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (extern $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (extern $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (mod $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (mod $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (static $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (static $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (const $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (const $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (trait $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (trait $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (unsafe trait $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (unsafe trait $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (impl $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (impl $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (unsafe impl $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (unsafe impl $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (fn $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (fn $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (unsafe fn $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (unsafe fn $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (type $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (type $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (enum $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (enum $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (struct $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (struct $($tail)*) -> $out $ctx)
    };
    
    // tree walker descends into token trees
    (@scan $paren:tt $life:tt $ret:ident ({ $($inner:tt)* } $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan {} $life $ret ($($inner)*) -> ()
               ($paren ($($tail)*) -> $out $ctx))
    };
    (@scan $paren:tt $life:tt $ret:ident (( $($inner:tt)* ) $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan () $life $ret ($($inner)*) -> ()
               ($paren ($($tail)*) -> $out $ctx))
    };
    (@scan $paren:tt $life:tt $ret:ident ([ $($inner:tt)* ] $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan [] $life $ret ($($inner)*) -> ()
               (($($tail)*) -> $out))
    };

    // fall-through case for tree walker: transfer over a token
    (@scan $paren:tt $life:tt $ret:ident ($head:tt $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* $head) $ctx)
    };

    // reformats arguments when popping a context off the tree walker stack
    // TODO this could be folded into the @scan rules that call it, to reduce recursion depth
    (@up $life:tt $ret:ident $thing:tt ($paren:tt $tail:tt -> ($($out:tt)*) $ctx:tt)) => {
        block!(@scan $paren $life $ret $tail -> ($($out)* $thing) $ctx)
    };
    
    ($life:tt: { $($body:tt)* }) => {
        block!(@scan {} $life _ret ($($body)*) -> () ())
        //      |    |  |     |    |              |  |
        //      |    |  |     |    |              |  ^ tree walker stack
        //      |    |  |     |    |              ^ transformed code
        //      |    |  |     |    ^ code to be transformed
        //      |    |  |     ^ block exit variable name (gensym)
        //      |    |  ^ block label
        //      |    ^ surrounding bracket type
        //      ^ start the tree walker!
    }
}

#[cfg(test)]
#[test]
fn it_works() {
    let flag = true;
    let x = block!('a: {
        if flag { break 'a "early exit"; }
        "normal exit"
    });
    assert_eq!(x, "early exit");

    let flag = false;
    let x = block!('b: {
        if flag { break 'b "early exit"; }
        let _y = block!('c: {
            if flag { break 'b "inner early exit"; };
            String::from("inner normal exit")
        });

        #[block(ignore)]
        {
            #[allow(dead_code)]
            fn f() -> i32 {
                block!('b: {
                    break 'b 42;
                })
            }
        }
        #[allow(dead_code)]
        fn g() {
            block!('b: {
                break 'b 42;
            });

            while false {
                break;
            }
            while false {
                continue;
            }
            'b: while false {
                continue 'b;
            }
        }

        enum Foo { Bar(i32) }
        let closure = move |Foo::Bar(x): Foo| -> i32 {
            x + block!('d: {
                break 'd 42;
            })
        };
        assert_eq!(closure(Foo::Bar(0)), 42);

        "normal exit"
    });
    assert_eq!(x, "normal exit");

    'e: for i in 1..5 {
        assert!(i >= 1 && i < 5);
        block!('d: {
            //continue; //~ERROR NoBareContinueInNamedBlock
            //continue 'd; //~ERROR NoMatchedContinueInNamedBlock
            continue 'e;
        });
    }
}

